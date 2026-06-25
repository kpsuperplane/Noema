use std::{collections::HashMap, path::PathBuf};

use crate::{
    memory::ParticipantRole,
    memory::Sensitivity,
    memory_extraction::{
        ExtractorMemoryProposal, ExtractorMemoryResponse, ValidatedMemoryProposal,
        build_memory_extraction_prompt, parse_memory_extraction_proposals,
        validate_memory_extraction_response,
    },
    memory_persistence::{
        ConversationItemKind, ConversationItemStatus, MemoryAuthorityLevel, MemoryExtractionMethod,
        NewConversation, NewConversationItem, NewConversationTurn, NewMemoryCandidate,
        NewMemoryParticipant, ObjectProvenanceSource, ObjectRef, ObjectType,
        SqliteMemoryRepository,
    },
    provider::GenerateOutputItem,
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
    protocol::{DaemonError, StartedConversation, TurnActivityStatus, TurnTranscriptItem},
};

#[derive(Debug, Clone)]
pub(super) struct CodexRuntimeHandle {
    sender: mpsc::Sender<CodexRuntimeCommand>,
}

impl CodexRuntimeHandle {
    pub(super) fn spawn(
        config: CodexProviderConfig,
        database_path: PathBuf,
    ) -> Result<Self, DaemonError> {
        let (sender, receiver) = mpsc::channel(16);
        let actor = CodexRuntimeActor::new(config, database_path)?;
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
        item_tx: mpsc::UnboundedSender<TurnTranscriptItem>,
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
        item_tx: mpsc::UnboundedSender<TurnTranscriptItem>,
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
    fn spawn(config: CodexProviderConfig, database_path: PathBuf) -> Result<Self, DaemonError> {
        let (sender, receiver) = mpsc::channel(16);
        let worker = MemoryExtractionWorker {
            runtime: CodexAppServerRuntime::new(config)?,
            memory_repository: SqliteMemoryRepository::open_at(database_path)?,
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
    memory_repository: SqliteMemoryRepository,
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
            &mut self.memory_repository,
            context,
            proposals,
            "ordinary_chat_extraction",
        )
    }
}

#[derive(Debug)]
struct CodexRuntimeActor {
    runtime: CodexAppServerRuntime,
    memory_extraction_worker: MemoryExtractionWorkerHandle,
    memory_repository: SqliteMemoryRepository,
    conversations: HashMap<String, ActiveConversation>,
}

impl CodexRuntimeActor {
    fn new(config: CodexProviderConfig, database_path: PathBuf) -> Result<Self, DaemonError> {
        Ok(Self {
            memory_extraction_worker: MemoryExtractionWorkerHandle::spawn(
                config.clone(),
                database_path.clone(),
            )?,
            runtime: CodexAppServerRuntime::new(config)?,
            memory_repository: SqliteMemoryRepository::open_at(database_path)?,
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
        self.memory_repository.ensure_default_actors()?;
        let conversation = self
            .runtime
            .start_conversation(model.clone(), cwd.clone())
            .await?;
        let provider_thread_id = conversation.thread_id.clone();
        let mut new_conversation = NewConversation::local_chat(model, cwd.clone());
        new_conversation.provider_thread_id = Some(provider_thread_id.clone());
        let durable_conversation = self
            .memory_repository
            .create_conversation(new_conversation)?;
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
        item_tx: mpsc::UnboundedSender<TurnTranscriptItem>,
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
            })?;
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
            })?;
        let saved_memory_id = self.persist_chat_memory_candidate(
            &conversation_id,
            turn_index,
            &user_item.item_id,
            &input,
        )?;
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
                let assistant_text = response.assistant_text();
                let provider_memory_proposals = response.memory_proposals();
                let mut assistant_item_id = None;
                for (index, output) in response.output.into_iter().enumerate() {
                    match output {
                        GenerateOutputItem::AssistantText { text } => {
                            let assistant_item = self.memory_repository.append_conversation_item(
                                NewConversationItem {
                                    conversation_id: conversation_id.clone(),
                                    turn_id: Some(turn.turn_id.clone()),
                                    parent_item_id: Some(user_item.item_id.clone()),
                                    kind: ConversationItemKind::AssistantText,
                                    status: ConversationItemStatus::Completed,
                                    author: ObjectRef::agent("agent:primary"),
                                    content_text: Some(text.clone()),
                                    payload_json: json!({}),
                                    metadata: json!({
                                        "turn_index": turn_index,
                                        "output_index": index,
                                    }),
                                },
                            )?;
                            if assistant_item_id.is_none() {
                                assistant_item_id = Some(assistant_item.item_id);
                            }
                            let _ = item_tx.send(TurnTranscriptItem::AssistantText { text });
                        }
                        GenerateOutputItem::MemoryProposals { .. } => {}
                        GenerateOutputItem::Structured { schema, payload } => {
                            let _ = self.memory_repository.append_conversation_item(
                                NewConversationItem {
                                    conversation_id: conversation_id.clone(),
                                    turn_id: Some(turn.turn_id.clone()),
                                    parent_item_id: Some(user_item.item_id.clone()),
                                    kind: ConversationItemKind::A2uiCard,
                                    status: ConversationItemStatus::Completed,
                                    author: ObjectRef::agent("agent:primary"),
                                    content_text: None,
                                    payload_json: json!({
                                        "schema": schema.clone(),
                                        "payload": payload.clone(),
                                    }),
                                    metadata: json!({
                                        "turn_index": turn_index,
                                        "output_index": index,
                                        "source": "provider_structured_output",
                                    }),
                                },
                            )?;
                            let _ = item_tx.send(TurnTranscriptItem::A2uiCard {
                                id: format!(
                                    "provider_structured:{conversation_id}:{turn_index}:{index}"
                                ),
                                schema: schema.clone(),
                                payload: payload.clone(),
                            });
                        }
                    }
                }
                self.memory_repository
                    .complete_conversation_turn(&turn.turn_id)?;

                if let Some(conversation) = self.conversations.get_mut(&conversation_id) {
                    conversation.next_turn_index = conversation.next_turn_index.saturating_add(1);
                }

                let memory_context = ConversationMemoryContext {
                    turn_index,
                    conversation_id: conversation_id.clone(),
                    turn_id: turn.turn_id,
                    user_item_id: user_item.item_id,
                    assistant_item_id,
                    user_content: input.clone(),
                    assistant_content: assistant_text.clone(),
                    cwd: conversation.cwd.clone(),
                };

                if let Some(memory_id) = saved_memory_id {
                    let _ = item_tx.send(typed_memory_activity(
                        &format!("memory_save:{conversation_id}:{turn_index}"),
                        "memory_save",
                        TurnActivityStatus::Completed,
                        "Memory saved",
                        Some("saved one explicit memory"),
                        json!({
                            "turn_index": turn_index,
                            "created_memory_ids": [memory_id],
                            "trigger": "explicit_remember",
                        }),
                    ));
                    let memory_content = explicit_memory_content(&input).unwrap_or_default();
                    let _ = item_tx.send(TurnTranscriptItem::A2uiCard {
                        id: format!("memory_cards:{conversation_id}:{turn_index}"),
                        schema: "memory_cards".to_string(),
                        payload: json!({
                            "turn_index": turn_index,
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
                    });
                } else if !provider_memory_proposals.is_empty() {
                    self.persist_provider_memory_proposals(
                        &memory_context,
                        provider_memory_proposals,
                        &item_tx,
                    );
                } else {
                    self.memory_extraction_worker.extract(memory_context).await;
                }

                Ok(())
            }
            Err(error) => {
                self.conversations.remove(&conversation_id);
                Err(error.into())
            }
        }
    }

    fn persist_provider_memory_proposals(
        &mut self,
        context: &ConversationMemoryContext,
        proposals: Vec<ExtractorMemoryProposal>,
        item_tx: &mpsc::UnboundedSender<TurnTranscriptItem>,
    ) {
        let turn_index = context.turn_index;
        let activity_id = format!("memory_extraction:{}:{turn_index}", context.conversation_id);
        let proposals = match validate_memory_extraction_response(
            ExtractorMemoryResponse { proposals },
            &context.user_content,
            &context.assistant_content,
        ) {
            Ok(proposals) => proposals,
            Err(error) => {
                let _ = item_tx.send(memory_activity_failed(
                    &activity_id,
                    format!("memory extraction output was rejected: {error}"),
                ));
                return;
            }
        };

        let card_proposals = proposals.clone();
        let created_memory_ids = match persist_validated_memory_proposals(
            &mut self.memory_repository,
            context,
            proposals,
            "provider_structured_output",
        ) {
            Ok(created_memory_ids) => created_memory_ids,
            Err(error) => {
                let _ = item_tx.send(memory_activity_failed(&activity_id, error));
                return;
            }
        };

        let _ = item_tx.send(TurnTranscriptItem::A2uiCard {
            id: format!("memory_proposals:{}:{turn_index}", context.conversation_id),
            schema: "memory_proposals".to_string(),
            payload: json!({
                "turn_index": turn_index,
                "source": "provider_structured_output",
                "created_memory_ids": created_memory_ids.clone(),
                "proposals": card_proposals,
            }),
        });

        let summary = match created_memory_ids.len() {
            0 => "created no memory candidates".to_string(),
            1 => "created 1 memory candidate".to_string(),
            count => format!("created {count} memory candidates"),
        };
        let _ = item_tx.send(memory_activity(
            &activity_id,
            TurnActivityStatus::Completed,
            "Memory extraction completed",
            Some(&summary),
            json!({
                "turn_index": turn_index,
                "created_memory_ids": created_memory_ids,
            }),
        ));
    }

    fn persist_chat_memory_candidate(
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

        let memory = self.memory_repository.append_memory_candidate(&candidate)?;
        Ok(Some(memory.id))
    }
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

fn persist_validated_memory_proposals(
    memory_repository: &mut SqliteMemoryRepository,
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
            .append_memory_candidate(&candidate)
            .map_err(|error| format!("failed to persist extracted memory: {error}"))?;
        created_memory_ids.push(summary.id);
    }

    Ok(created_memory_ids)
}
