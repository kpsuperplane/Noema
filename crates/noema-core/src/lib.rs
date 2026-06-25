//! Core Noema types and runtimes.
//!
//! This crate contains configuration loading, provider adapters, daemon
//! protocol support, home-directory setup, path resolution, and the V1 memory
//! retrieval model.

/// Configuration loading and provider selection.
pub mod config;
/// Persisted context graph inspection view.
pub mod context_graph;
mod context_graph_rows;
mod context_graph_sql;
/// Local daemon protocol and client/server runtime.
pub mod daemon;
/// Rust-owned protocol types exported to the web frontend.
pub mod frontend_protocol;
/// Noema home-directory initialization.
pub mod home;
/// V1 memory storage and retrieval policy model.
pub mod memory;
/// Pure ordinary-chat memory extraction proposal layer.
pub mod memory_extraction;
/// SQLite-backed durable memory repository.
pub mod memory_persistence;
/// Filesystem path resolution for Noema state.
pub mod paths;
/// Provider-neutral generation request and response types.
pub mod provider;
/// Concrete model provider adapters.
pub mod providers;
mod retrieval_policy_fingerprint;
mod sqlite_memory_retrieval;

pub use config::{
    CliOverrides, Config, ConfigError, DaemonResolvedConfig, ProviderConfig, ProviderKind,
    ResolvedConfig, WebConfig,
};
pub use context_graph::{
    ContextGraphFilter, ContextGraphSummary, GraphAccessGrant, GraphContextPacket,
    GraphContextPacketMemoryEdge, GraphContextPacketOmission, GraphEntityNode, GraphMemoryEvent,
    GraphMemoryNode, GraphMemoryUseRecord, GraphObjectLinkEdge, GraphParticipantEdge,
    GraphProvenanceEdge, GraphPurposeRule, GraphSubjectEdge, RelationshipSummary,
};
pub use daemon::{
    DaemonClient, DaemonError, DaemonRequest, DaemonResponse, DaemonServerConfig,
    StartedConversation, TurnActivityStatus, TurnTranscriptItem, default_socket_path,
    is_connection_refused, run_daemon, socket_path_for_home,
};
pub use home::{
    DEFAULT_NOEMA_CONFIG_YAML, NoemaHomeError, NoemaHomeInitOptions, NoemaHomeInitResult,
    init_noema_home,
};
pub use memory_extraction::{
    ExtractorMemoryProposal, ExtractorMemoryResponse, MIN_MEMORY_EXTRACTION_CONFIDENCE,
    MemoryExtractionError, MemoryExtractionRetrievalHints, MemoryExtractionRiskFlag,
    MemoryExtractionSubject, MemoryExtractionSubjectKind, MemoryExtractionSubjectRole,
    ValidatedMemoryProposal, build_memory_extraction_prompt, decide_memory_proposal_status,
    parse_memory_extraction_proposals,
};
pub use memory_persistence::{
    ChatMemorySource, MemoryAuthorityLevel, MemoryExtractionMethod, MemoryPersistenceError,
    MemorySummary, MemoryType, NewChatMemoryCandidate, NewChatTurn, NewMemoryParticipant,
    NewMemorySubject, NewRelationshipClaim, SqliteMemoryRepository,
};
pub use paths::{NOEMA_HOME_ENV, NoemaPathError, NoemaPaths};
pub use provider::{
    GenerateInput, GenerateOptions, GenerateRequest, GenerateResponse, ModelProvider,
    ProviderError, TokenUsage,
};
pub use providers::{
    codex::{CodexProvider, CodexProviderConfig},
    codex_app_server::{CodexAppServerConversation, CodexAppServerRuntime},
    openai::{OpenAiProvider, OpenAiProviderConfig},
};
