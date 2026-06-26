use std::collections::HashMap;

use crate::{
    DatabaseConfig,
    memory::ParticipantRole,
    memory::Sensitivity,
    memory_extraction::{
        ExtractorMemoryProposal, ExtractorMemoryResponse, ValidatedMemoryProposal,
        build_memory_extraction_prompt, parse_memory_extraction_proposals,
        validate_memory_extraction_response,
    },
    memory_persistence::{
        AgentStatus as PersistedAgentStatus, ConversationItemKind, ConversationItemRecord,
        ConversationItemStatus, MemoryAuthorityLevel, MemoryExtractionMethod, NewConversation,
        NewConversationItem, NewConversationTurn, NewMemoryCandidate, NewMemoryParticipant,
        ObjectProvenanceSource, ObjectRef, ObjectType, PostgresMemoryRepository,
    },
    provider::{GenerateOutputItem, GenerateResponse},
    providers::{
        codex::CodexProviderConfig,
        codex_app_server::{CodexAppServerConversation, CodexAppServerRuntime},
    },
};
use serde_json::json;
use tokio::sync::{mpsc, oneshot};

use super::{
    memory_pipeline::{
        ConversationMemoryContext, explicit_memory_content, extracted_proposal_to_candidate,
        infer_chat_memory_type, infer_chat_sensitivity, memory_activity, memory_activity_failed,
        project_scope_from_cwd, title_from_memory_content, typed_memory_activity,
    },
    protocol::{
        AgentStatus, DaemonError, StartedConversation, TurnActivityStatus, TurnStreamEvent,
        TurnTranscriptItem,
    },
};

#[derive(Debug, Clone)]
pub(super) struct CodexRuntimeHandle {
    sender: mpsc::Sender<CodexRuntimeCommand>,
}

impl CodexRuntimeHandle {
    pub(super) async fn spawn(
        config: CodexProviderConfig,
        database_url: String,
    ) -> Result<Self, DaemonError> {
        let (sender, receiver) = mpsc::channel(16);
        let actor = CodexRuntimeActor::new(config, database_url).await?;
        tokio::spawn(actor.run(receiver));
        Ok(Self { sender })
    }

    pub(super) async fn start_conversation(
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

    pub(super) async fn turn(
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

    pub(super) async fn end_conversation(
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

    pub(super) async fn shutdown(&self) {
        let (reply, reply_rx) = oneshot::channel();
        let _ = self
            .sender
            .send(CodexRuntimeCommand::Shutdown { reply })
            .await;
        let _ = reply_rx.await;
    }
}

#[derive(Debug)]
enum CodexRuntimeCommand {
    StartConversation {
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
    async fn spawn(config: CodexProviderConfig, database_url: String) -> Result<Self, DaemonError> {
        let (sender, receiver) = mpsc::channel(16);
        let database = DatabaseConfig::new(database_url)?;
        let worker = MemoryExtractionWorker {
            runtime: CodexAppServerRuntime::new(config)?,
            memory_repository: PostgresMemoryRepository::connect(&database).await?,
        };
        tokio::spawn(worker.run(receiver));
        Ok(Self { sender })
    }

    async fn extract(&self, context: ConversationMemoryContext) {
        let _ = self
            .sender
            .send(MemoryExtractionWorkerCommand::Extract(Box::new(context)))
            .await;
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
    Extract(Box<ConversationMemoryContext>),
    Shutdown { reply: oneshot::Sender<()> },
}

#[derive(Debug)]
struct MemoryExtractionWorker {
    runtime: CodexAppServerRuntime,
    memory_repository: PostgresMemoryRepository,
}

impl MemoryExtractionWorker {
    async fn run(mut self, mut receiver: mpsc::Receiver<MemoryExtractionWorkerCommand>) {
        while let Some(command) = receiver.recv().await {
            match command {
                MemoryExtractionWorkerCommand::Extract(context) => {
                    let _ = self.extract_ordinary_chat_memories(&context).await;
                }
                MemoryExtractionWorkerCommand::Shutdown { reply } => {
                    self.runtime.shutdown().await;
                    let _ = reply.send(());
                    break;
                }
            }
        }
    }

    async fn extract_ordinary_chat_memories(
        &mut self,
        context: &ConversationMemoryContext,
    ) -> Result<Vec<String>, String> {
        let project_hint = project_scope_from_cwd(context.cwd.as_deref());
        let prompt = build_memory_extraction_prompt(
            &context.user_content,
            &context.assistant_content,
            &context.conversation_id,
            context.turn_index,
            project_hint.as_deref(),
        );

        let extraction_conversation = self
            .runtime
            .start_conversation(None, context.cwd.clone())
            .await
            .map_err(|error| format!("memory extraction model failed: {error}"))?;
        let extraction_text = self
            .runtime
            .turn(&extraction_conversation, prompt)
            .await
            .map_err(|error| format!("memory extraction model failed: {error}"))?
            .assistant_text();

        let proposals = parse_memory_extraction_proposals(
            &extraction_text,
            &context.user_content,
            &context.assistant_content,
        )
        .map_err(|error| format!("memory extraction output was rejected: {error}"))?;

        persist_validated_memory_proposals(
            &self.memory_repository,
            context,
            proposals,
            "ordinary_chat_extraction",
        )
        .await
    }
}

#[derive(Debug)]
struct CodexRuntimeActor {
    runtime: CodexAppServerRuntime,
    memory_extraction_worker: MemoryExtractionWorkerHandle,
    memory_repository: PostgresMemoryRepository,
    conversations: HashMap<String, ActiveConversation>,
}

impl CodexRuntimeActor {
    async fn new(config: CodexProviderConfig, database_url: String) -> Result<Self, DaemonError> {
        let database = DatabaseConfig::new(database_url.clone())?;
        Ok(Self {
            memory_extraction_worker: MemoryExtractionWorkerHandle::spawn(
                config.clone(),
                database_url,
            )
            .await?,
            runtime: CodexAppServerRuntime::new(config)?,
            memory_repository: PostgresMemoryRepository::connect(&database).await?,
            conversations: HashMap::new(),
        })
    }

    async fn run(mut self, mut receiver: mpsc::Receiver<CodexRuntimeCommand>) {
        while let Some(command) = receiver.recv().await {
            match command {
                CodexRuntimeCommand::StartConversation { model, cwd, reply } => {
                    let _ = reply.send(self.start_conversation(model, cwd).await);
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
                    self.runtime.shutdown().await;
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
        self.memory_repository.ensure_default_actors().await?;
        let conversation = self
            .runtime
            .start_conversation(model.clone(), cwd.clone())
            .await?;
        let provider_thread_id = conversation.thread_id.clone();
        let mut new_conversation = NewConversation::local_chat(model, cwd.clone());
        new_conversation.provider_thread_id = Some(provider_thread_id.clone());
        let durable_conversation = self
            .memory_repository
            .create_conversation(new_conversation)
            .await?;
        let conversation_id = durable_conversation.conversation_id;
        self.conversations.insert(
            conversation_id.clone(),
            ActiveConversation {
                provider: conversation,
                cwd,
                next_turn_index: 1,
            },
        );

        Ok(StartedConversation {
            conversation_id,
            provider_thread_id,
        })
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
            .memory_repository
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({ "turn_index": turn_index }),
            })
            .await?;
        self.update_conversation_agent_status(
            &conversation_id,
            PersistedAgentStatus::InputReceived,
            &item_tx,
        )
        .await?;
        let user_item = self
            .memory_repository
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                author: ObjectRef::human("human:local"),
                content_text: Some(input.clone()),
                payload_json: json!({}),
                metadata: json!({ "turn_index": turn_index }),
            })
            .await?;
        let user_item_id = user_item.item_id.clone();
        send_conversation_item(
            &item_tx,
            user_item,
            TurnTranscriptItem::UserText {
                text: input.clone(),
            },
        );
        let saved_memory_id = self
            .persist_chat_memory_candidate(&conversation_id, turn_index, &user_item_id, &input)
            .await?;
        if let Some(memory_id) = saved_memory_id.as_deref() {
            let memory_context = ConversationMemoryContext {
                turn_index,
                conversation_id: conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                user_item_id: user_item_id.clone(),
                assistant_item_id: None,
                user_content: input.clone(),
                assistant_content: String::new(),
                cwd: conversation.cwd.clone(),
            };
            if let Err(error) = self
                .persist_explicit_memory_transcript_items(
                    &memory_context,
                    memory_id,
                    &input,
                    &item_tx,
                )
                .await
            {
                self.record_turn_failure(&memory_context, error.to_string(), &item_tx)
                    .await?;
                self.conversations.remove(&conversation_id);
                return Err(error);
            }
        }
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
        );

        match self
            .runtime
            .turn_structured(
                &conversation.provider,
                input.clone(),
                structured_instructions,
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
                            response,
                            saved_memory_id,
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
                        user_content: input,
                        assistant_content: String::new(),
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
                let error_context = ConversationMemoryContext {
                    turn_index,
                    conversation_id: conversation_id.clone(),
                    turn_id: turn.turn_id,
                    user_item_id,
                    assistant_item_id: None,
                    user_content: input,
                    assistant_content: String::new(),
                    cwd: conversation.cwd.clone(),
                };
                self.record_turn_failure(&error_context, error.to_string(), &item_tx)
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
        let assistant_text = turn.response.assistant_text();
        let provider_memory_proposals = turn.response.memory_proposals();
        let mut assistant_item_id = None;
        for (index, output) in turn.response.output.into_iter().enumerate() {
            match output {
                GenerateOutputItem::AssistantText { text } => {
                    let assistant_item = self
                        .memory_repository
                        .append_conversation_item(NewConversationItem {
                            conversation_id: turn.conversation_id.clone(),
                            turn_id: Some(turn.turn_id.clone()),
                            parent_item_id: Some(turn.user_item_id.clone()),
                            kind: ConversationItemKind::AssistantText,
                            status: ConversationItemStatus::Completed,
                            author: ObjectRef::agent("agent:primary"),
                            content_text: Some(text.clone()),
                            payload_json: json!({}),
                            metadata: json!({
                                "turn_index": turn.turn_index,
                                "output_index": index,
                            }),
                        })
                        .await?;
                    if assistant_item_id.is_none() {
                        assistant_item_id = Some(assistant_item.item_id.clone());
                    }
                    send_conversation_item(
                        item_tx,
                        assistant_item,
                        TurnTranscriptItem::AssistantText { text },
                    );
                }
                GenerateOutputItem::MemoryProposals { .. } => {}
                GenerateOutputItem::Structured { schema, payload } => {
                    let card_id = format!(
                        "provider_structured:{}:{}:{index}",
                        turn.conversation_id, turn.turn_index
                    );
                    let structured_item = self
                        .memory_repository
                        .append_conversation_item(NewConversationItem {
                            conversation_id: turn.conversation_id.clone(),
                            turn_id: Some(turn.turn_id.clone()),
                            parent_item_id: Some(turn.user_item_id.clone()),
                            kind: ConversationItemKind::A2uiCard,
                            status: ConversationItemStatus::Completed,
                            author: ObjectRef::agent("agent:primary"),
                            content_text: None,
                            payload_json: json!({
                                "id": card_id.clone(),
                                "schema": schema.clone(),
                                "payload": payload.clone(),
                            }),
                            metadata: json!({
                                "turn_index": turn.turn_index,
                                "output_index": index,
                                "source": "provider_structured_output",
                            }),
                        })
                        .await?;
                    send_conversation_item(
                        item_tx,
                        structured_item,
                        TurnTranscriptItem::A2uiCard {
                            id: card_id,
                            schema: schema.clone(),
                            payload: payload.clone(),
                        },
                    );
                }
            }
        }

        let memory_context = ConversationMemoryContext {
            turn_index: turn.turn_index,
            conversation_id: turn.conversation_id.clone(),
            turn_id: turn.turn_id.clone(),
            user_item_id: turn.user_item_id.clone(),
            assistant_item_id,
            user_content: turn.user_input,
            assistant_content: assistant_text,
            cwd: turn.cwd,
        };

        if turn.saved_memory_id.is_none() && !provider_memory_proposals.is_empty() {
            self.persist_provider_memory_proposals(
                &memory_context,
                provider_memory_proposals,
                item_tx,
            )
            .await?;
        } else if turn.saved_memory_id.is_none() {
            self.memory_extraction_worker.extract(memory_context).await;
        }

        self.memory_repository
            .complete_conversation_turn(&turn.turn_id)
            .await?;
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

    async fn persist_explicit_memory_transcript_items(
        &mut self,
        context: &ConversationMemoryContext,
        memory_id: &str,
        user_input: &str,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let activity = typed_memory_activity(
            &format!(
                "memory_save:{}:{}",
                context.conversation_id, context.turn_index
            ),
            "memory_save",
            TurnActivityStatus::Completed,
            "Memory saved",
            Some("saved one explicit memory"),
            json!({
                "turn_index": context.turn_index,
                "created_memory_ids": [memory_id],
                "trigger": "explicit_remember",
            }),
        );
        self.persist_and_send_turn_item(context, activity, item_tx)
            .await?;

        let memory_content = explicit_memory_content(user_input).unwrap_or_default();
        let card = TurnTranscriptItem::A2uiCard {
            id: format!(
                "memory_cards:{}:{}",
                context.conversation_id, context.turn_index
            ),
            schema: "memory_cards".to_string(),
            payload: json!({
                "turn_index": context.turn_index,
                "source": "explicit_remember",
                "created_memory_ids": [memory_id],
                "memories": [{
                    "content": memory_content,
                    "title": title_from_memory_content(&memory_content),
                    "memory_type": infer_chat_memory_type(&memory_content).as_str(),
                    "sensitivity": sensitivity_payload_label(infer_chat_sensitivity(&memory_content)),
                    "status": "confirmed",
                }],
            }),
        };
        self.persist_and_send_turn_item(context, card, item_tx)
            .await
    }

    async fn record_turn_failure(
        &mut self,
        context: &ConversationMemoryContext,
        message: String,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        self.memory_repository
            .fail_conversation_turn(&context.turn_id)
            .await?;
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
        context: &ConversationMemoryContext,
        proposals: Vec<ExtractorMemoryProposal>,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let turn_index = context.turn_index;
        let activity_id = format!("memory_extraction:{}:{turn_index}", context.conversation_id);
        let proposals = match validate_memory_extraction_response(
            ExtractorMemoryResponse { proposals },
            &context.user_content,
            &context.assistant_content,
        ) {
            Ok(proposals) => proposals,
            Err(error) => {
                let activity = memory_activity_failed(
                    &activity_id,
                    format!("memory extraction output was rejected: {error}"),
                );
                self.persist_and_send_turn_item(context, activity, item_tx)
                    .await?;
                return Ok(());
            }
        };

        let card_proposals = proposals.clone();
        let created_memory_ids = match persist_validated_memory_proposals(
            &self.memory_repository,
            context,
            proposals,
            "provider_structured_output",
        )
        .await
        {
            Ok(created_memory_ids) => created_memory_ids,
            Err(error) => {
                let activity = memory_activity_failed(&activity_id, error);
                self.persist_and_send_turn_item(context, activity, item_tx)
                    .await?;
                return Ok(());
            }
        };

        let card = TurnTranscriptItem::A2uiCard {
            id: format!("memory_proposals:{}:{turn_index}", context.conversation_id),
            schema: "memory_proposals".to_string(),
            payload: json!({
                "turn_index": turn_index,
                "source": "provider_structured_output",
                "created_memory_ids": created_memory_ids.clone(),
                "proposals": card_proposals,
            }),
        };
        self.persist_and_send_turn_item(context, card, item_tx)
            .await?;

        let summary = match created_memory_ids.len() {
            0 => "created no memory candidates".to_string(),
            1 => "created 1 memory candidate".to_string(),
            count => format!("created {count} memory candidates"),
        };
        let activity = memory_activity(
            &activity_id,
            TurnActivityStatus::Completed,
            "Memory extraction completed",
            Some(&summary),
            json!({
                "turn_index": turn_index,
                "created_memory_ids": created_memory_ids,
            }),
        );
        self.persist_and_send_turn_item(context, activity, item_tx)
            .await
    }

    async fn persist_and_send_turn_item(
        &mut self,
        context: &ConversationMemoryContext,
        item: TurnTranscriptItem,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        let record = self.persist_turn_item(context, &item).await?;
        send_conversation_item(item_tx, record, item);
        Ok(())
    }

    async fn persist_turn_item(
        &mut self,
        context: &ConversationMemoryContext,
        item: &TurnTranscriptItem,
    ) -> Result<ConversationItemRecord, DaemonError> {
        let default_parent_item_id = context
            .assistant_item_id
            .clone()
            .or_else(|| Some(context.user_item_id.clone()));
        let (kind, status, author, parent_item_id, content_text, payload_json, metadata) =
            match item {
                TurnTranscriptItem::UserText { text } => (
                    ConversationItemKind::UserText,
                    ConversationItemStatus::Completed,
                    ObjectRef::human("human:local"),
                    None,
                    Some(text.clone()),
                    json!({}),
                    json!({ "turn_index": context.turn_index }),
                ),
                TurnTranscriptItem::AssistantText { text } => (
                    ConversationItemKind::AssistantText,
                    ConversationItemStatus::Completed,
                    ObjectRef::agent("agent:primary"),
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
                    ObjectRef::agent("agent:primary"),
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
                    ObjectRef::agent("agent:primary"),
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
                    ObjectRef::agent("agent:primary"),
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
            .memory_repository
            .append_conversation_item(NewConversationItem {
                conversation_id: context.conversation_id.clone(),
                turn_id: Some(context.turn_id.clone()),
                parent_item_id,
                kind,
                status,
                author,
                content_text,
                payload_json,
                metadata,
            })
            .await?;
        Ok(record)
    }

    async fn persist_chat_memory_candidate(
        &mut self,
        conversation_id: &str,
        turn_index: u64,
        user_item_id: &str,
        user_input: &str,
    ) -> Result<Option<String>, DaemonError> {
        let Some(memory_content) = explicit_memory_content(user_input) else {
            return Ok(None);
        };

        let mut candidate = NewMemoryCandidate::confirmed_note(
            ObjectRef::new(ObjectType::Conversation, conversation_id)?,
            memory_content,
            ObjectRef::human("human:local"),
            ObjectRef::conversation_item(user_item_id),
        );
        candidate.memory_type = infer_chat_memory_type(&candidate.content);
        candidate.title = Some(title_from_memory_content(&candidate.content));
        candidate.sensitivity = infer_chat_sensitivity(&candidate.content);
        candidate.status = crate::memory::MemoryStatus::Confirmed;
        candidate.owner_actor = Some(ObjectRef::human("human:local"));
        candidate.authority_level = MemoryAuthorityLevel::ExplicitHumanStatement;
        candidate.extraction_method = MemoryExtractionMethod::ExplicitHuman;
        candidate.source = Some(ObjectProvenanceSource {
            source: ObjectRef::conversation_item(user_item_id),
            evidence_excerpt: Some(user_input.trim().to_string()),
        });
        candidate.participants = vec![
            NewMemoryParticipant::new("human:local", ParticipantRole::HumanInScope),
            NewMemoryParticipant::new("agent:primary", ParticipantRole::AgentInScope),
        ];
        candidate.metadata = json!({
            "trigger": "explicit_remember",
            "turn_index": turn_index,
        });

        let memory = self
            .memory_repository
            .append_memory_candidate(candidate)
            .await?;
        Ok(Some(memory.id))
    }

    async fn update_conversation_agent_status(
        &mut self,
        conversation_id: &str,
        status: PersistedAgentStatus,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        self.memory_repository
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
    item: TurnTranscriptItem,
) {
    let _ = item_tx.send(TurnStreamEvent::ConversationItem {
        conversation_id: record.conversation_id,
        item_id: record.item_id,
        turn_id: record.turn_id,
        item,
    });
}

const fn sensitivity_payload_label(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
    }
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

const fn activity_status_payload(status: TurnActivityStatus) -> &'static str {
    match status {
        TurnActivityStatus::Started => "started",
        TurnActivityStatus::Completed => "completed",
        TurnActivityStatus::Failed => "failed",
    }
}

fn build_structured_turn_system_prompt(
    conversation_id: &str,
    turn_index: u64,
    cwd: Option<&str>,
) -> String {
    let project_hint = project_scope_from_cwd(cwd).unwrap_or_else(|| "none".to_string());

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
  "evidence_excerpt": "exact contiguous quote from the user message"
}}

Rules:
- Always include exactly one assistant_text item.
- Include exactly one memory_proposals item. Use an empty proposals array when there are no durable memories.
- Propose only durable facts, preferences, constraints, decisions, routines, goals, procedures, or notes that could matter later.
- Do not propose jokes, speculation, transient task chatter, or generic world facts.
- evidence_excerpt must be an exact contiguous quote from the user message and directly support the proposal.
- subjects must be non-empty and must show a human subject or participant when the memory affects a person.
- Use id "human:local" only for the current human/user/me. Do not use it for third-party people.
- confidence must be between 0.0 and 1.0. Use at least 0.70 only when evidence directly supports the proposal.
- Use an empty risk_flags array only for low-risk direct ordinary facts and preferences.
- Add risk_flags for inferred, sensitive, secret, action-triggering, contradiction-prone, third-party, risk-bearing, temporary, or external-egress proposals.

Conversation metadata:
conversation_id: {conversation_id}
turn_index: {turn_index}
cwd_project_hint: {project_hint}"#
    )
}

#[derive(Debug, Clone)]
struct ActiveConversation {
    provider: CodexAppServerConversation,
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
    response: GenerateResponse,
    saved_memory_id: Option<String>,
}

async fn persist_validated_memory_proposals(
    memory_repository: &PostgresMemoryRepository,
    context: &ConversationMemoryContext,
    proposals: Vec<ValidatedMemoryProposal>,
    trigger: &str,
) -> Result<Vec<String>, String> {
    let project_hint = project_scope_from_cwd(context.cwd.as_deref());
    let mut created_memory_ids = Vec::new();

    for proposal in proposals {
        let candidate = extracted_proposal_to_candidate(
            &proposal,
            context,
            project_hint.as_deref(),
            &context.user_content,
            trigger,
        )?;

        let summary = memory_repository
            .append_memory_candidate(candidate)
            .await
            .map_err(|error| format!("failed to persist extracted memory: {error}"))?;
        created_memory_ids.push(summary.id);
    }

    Ok(created_memory_ids)
}
