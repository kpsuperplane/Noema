use std::{collections::HashMap, path::PathBuf};

use crate::{
    memory::ParticipantRole,
    memory_extraction::{build_memory_extraction_prompt, parse_memory_extraction_proposals},
    memory_persistence::{
        ChatMemorySource, MemoryAuthorityLevel, MemoryExtractionMethod, NewChatMemoryCandidate,
        NewChatTurn, NewMemoryParticipant, SqliteMemoryRepository,
    },
    providers::{
        codex::CodexProviderConfig,
        codex_app_server::{CodexAppServerConversation, CodexAppServerRuntime},
    },
};
use serde_json::json;
use tokio::sync::{mpsc, oneshot};

use super::{
    memory_pipeline::{
        explicit_memory_content, extracted_proposal_to_candidate, infer_chat_memory_type,
        infer_chat_sensitivity, memory_activity, memory_activity_failed, project_scope_from_cwd,
        title_from_memory_content,
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

#[derive(Debug)]
struct CodexRuntimeActor {
    runtime: CodexAppServerRuntime,
    memory_extraction_runtime: CodexAppServerRuntime,
    memory_repository: SqliteMemoryRepository,
    conversations: HashMap<String, ActiveConversation>,
    next_conversation_id: u64,
}

impl CodexRuntimeActor {
    fn new(config: CodexProviderConfig, database_path: PathBuf) -> Result<Self, DaemonError> {
        Ok(Self {
            memory_extraction_runtime: CodexAppServerRuntime::new(config.clone())?,
            runtime: CodexAppServerRuntime::new(config)?,
            memory_repository: SqliteMemoryRepository::open_at(database_path)?,
            conversations: HashMap::new(),
            next_conversation_id: 1,
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
                    self.memory_extraction_runtime.shutdown().await;
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
        let conversation = self.runtime.start_conversation(model, cwd.clone()).await?;
        let conversation_id = format!("conversation_{}", self.next_conversation_id);
        self.next_conversation_id += 1;
        let provider_thread_id = conversation.thread_id.clone();
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
        let is_explicit_memory = explicit_memory_content(&input).is_some();

        self.persist_chat_memory_candidate(&conversation_id, turn_index, &input)?;

        match self
            .runtime
            .turn(&conversation.provider, input.clone())
            .await
        {
            Ok(response) => {
                let assistant_text = response.text;
                let _ = item_tx.send(TurnTranscriptItem::AssistantText {
                    text: assistant_text.clone(),
                });

                if let Some(conversation) = self.conversations.get_mut(&conversation_id) {
                    conversation.next_turn_index = conversation.next_turn_index.saturating_add(1);
                }

                let turn = NewChatTurn::new(
                    format!("conversation:{conversation_id}"),
                    turn_index,
                    "human:local",
                    "agent:primary",
                    input.clone(),
                    assistant_text.clone(),
                );
                if let Err(error) = self.memory_repository.record_chat_turn(&turn) {
                    let _ = item_tx.send(memory_activity_failed(
                        "memory_extraction:failed",
                        format!("failed to record chat turn provenance: {error}"),
                    ));
                    return Ok(());
                }

                if !is_explicit_memory {
                    self.extract_ordinary_chat_memories(
                        &conversation_id,
                        conversation.cwd.as_deref(),
                        &turn,
                        &item_tx,
                    )
                    .await;
                }

                Ok(())
            }
            Err(error) => {
                self.conversations.remove(&conversation_id);
                Err(error.into())
            }
        }
    }

    async fn extract_ordinary_chat_memories(
        &mut self,
        conversation_id: &str,
        cwd: Option<&str>,
        turn: &NewChatTurn,
        item_tx: &mpsc::UnboundedSender<TurnTranscriptItem>,
    ) {
        let turn_index = turn.turn_index;
        let activity_id = format!("memory_extraction:{conversation_id}:{turn_index}");
        let _ = item_tx.send(memory_activity(
            &activity_id,
            TurnActivityStatus::Started,
            "Extracting memory proposals",
            Some("ordinary chat memory extraction is running"),
            json!({ "turn_index": turn_index }),
        ));

        let project_hint = project_scope_from_cwd(cwd);
        let prompt = build_memory_extraction_prompt(
            &turn.user_content,
            &turn.assistant_content,
            conversation_id,
            turn_index,
            project_hint.as_deref(),
        );

        let extraction = match self
            .memory_extraction_runtime
            .start_conversation(None, cwd.map(ToOwned::to_owned))
            .await
        {
            Ok(extraction_conversation) => {
                self.memory_extraction_runtime
                    .turn(&extraction_conversation, prompt)
                    .await
            }
            Err(error) => Err(error),
        };

        let extraction_text = match extraction {
            Ok(response) => response.text,
            Err(error) => {
                let _ = item_tx.send(memory_activity_failed(
                    &activity_id,
                    format!("memory extraction model failed: {error}"),
                ));
                return;
            }
        };

        let proposals = match parse_memory_extraction_proposals(
            &extraction_text,
            &turn.user_content,
            &turn.assistant_content,
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

        let conversation_scope_id = format!("conversation:{conversation_id}");
        let mut created_memory_ids = Vec::new();
        for proposal in proposals {
            let candidate = match extracted_proposal_to_candidate(
                &proposal,
                &conversation_scope_id,
                project_hint.as_deref(),
                turn,
                &turn.user_content,
            ) {
                Ok(candidate) => candidate,
                Err(error) => {
                    let _ = item_tx.send(memory_activity_failed(&activity_id, error));
                    return;
                }
            };

            match self
                .memory_repository
                .append_chat_memory_candidate(&candidate)
            {
                Ok(summary) => created_memory_ids.push(summary.id),
                Err(error) => {
                    let _ = item_tx.send(memory_activity_failed(
                        &activity_id,
                        format!("failed to persist extracted memory: {error}"),
                    ));
                    return;
                }
            }
        }

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
        user_input: &str,
    ) -> Result<(), DaemonError> {
        let Some(memory_content) = explicit_memory_content(user_input) else {
            return Ok(());
        };

        let conversation_scope_id = format!("conversation:{conversation_id}");
        let message_id = format!("message:{conversation_scope_id}:user:{turn_index}");
        let mut candidate = NewChatMemoryCandidate::new(
            conversation_scope_id.clone(),
            memory_content,
            "human:local",
        );
        candidate.memory_type = infer_chat_memory_type(&candidate.content);
        candidate.title = Some(title_from_memory_content(&candidate.content));
        candidate.sensitivity = infer_chat_sensitivity(&candidate.content);
        candidate.status = crate::memory::MemoryStatus::Confirmed;
        candidate.owner_principal_id = Some("human:local".to_string());
        candidate.authority_level = MemoryAuthorityLevel::ExplicitHumanStatement;
        candidate.extraction_method = MemoryExtractionMethod::ExplicitHuman;
        candidate.source = Some(ChatMemorySource {
            conversation_id: conversation_scope_id,
            message_id: Some(message_id),
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

        self.memory_repository
            .append_chat_memory_candidate(&candidate)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ActiveConversation {
    provider: CodexAppServerConversation,
    cwd: Option<String>,
    next_turn_index: u64,
}
