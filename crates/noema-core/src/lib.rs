//! Core Noema types and runtimes.
//!
//! This crate contains configuration loading, provider adapters, daemon
//! protocol support, home-directory setup, path resolution, and the memory
//! retrieval model.

/// Configuration loading and provider selection.
pub mod config;
/// Persisted context graph inspection view.
pub mod context_graph;
/// Local daemon protocol and client/server runtime.
pub mod daemon;
/// Database configuration and Postgres connection helpers.
pub mod database;
/// GraphQL client API facade.
pub mod graphql;
/// Noema home-directory initialization.
pub mod home;
/// Memory storage and retrieval policy model.
pub mod memory;
/// Pure ordinary-chat memory extraction proposal layer.
pub mod memory_extraction;
/// Durable memory repositories and Postgres migration bootstrap.
pub mod memory_persistence;
/// Onboarding status derived from provider account readiness.
pub mod onboarding;
/// Filesystem path resolution for Noema state.
pub mod paths;
mod postgres_memory_retrieval;
mod postgres_retrieval_policy_fingerprint;
/// Provider-neutral generation request and response types.
pub mod provider;
/// Provider authentication support.
pub mod provider_auth;
/// Concrete model provider adapters.
pub mod providers;
mod retrieval_policy_fingerprint;
/// Embedded canonical structured store.
pub mod store;

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
    AgentStatus, DaemonClient, DaemonError, DaemonRequest, DaemonResponse, DaemonServerConfig,
    StartedConversation, TurnActivityStatus, TurnTranscriptItem, default_socket_path,
    is_connection_refused, run_daemon, socket_path_for_home,
};
pub use database::{DatabaseConfig, DatabaseConfigError, NOEMA_DATABASE_URL_ENV};
pub use home::{
    DEFAULT_NOEMA_CONFIG_YAML, NoemaHomeError, NoemaHomeInitOptions, NoemaHomeInitResult,
    init_noema_home,
};
pub use memory_extraction::{
    ExtractorMemoryProposal, ExtractorMemoryResponse, MIN_MEMORY_EXTRACTION_CONFIDENCE,
    MemoryExtractionError, MemoryExtractionRetrievalHints, MemoryExtractionRiskFlag,
    MemoryExtractionSubject, MemoryExtractionSubjectKind, MemoryExtractionSubjectRole,
    ValidatedMemoryProposal, build_memory_extraction_prompt, decide_memory_proposal_status,
    parse_memory_extraction_proposals, validate_memory_extraction_response,
};
pub use memory_persistence::{
    ActorRef, ConversationItemKind, ConversationItemRecord, ConversationItemStatus,
    ConversationRecord, ConversationTurnRecord, ConversationTurnStatus, DeleteConversationItem,
    MemoryAuthorityLevel, MemoryExtractionMethod, MemoryPersistenceError, MemorySummary,
    MemoryType, NewConversation, NewConversationItem, NewConversationTurn, NewMemoryCandidate,
    NewMemoryParticipant, NewMemorySubject, NewObjectProvenanceEdge, ObjectProvenanceSource,
    ObjectRef, ObjectType, PostgresMemoryRepository, ProviderAccountRecord, ProviderAccountStatus,
    ProviderAuthMethod, ReplayMode,
};
pub use onboarding::{
    OnboardingStatus, OnboardingStep, OnboardingStepStatus, onboarding_status_from_account,
};
pub use paths::{NOEMA_HOME_ENV, NoemaPathError, NoemaPaths};
pub use provider::{
    GenerateInput, GenerateOptions, GenerateOutputItem, GenerateRequest, GenerateResponse,
    ModelProvider, ProviderError, TokenUsage,
};
pub use providers::{
    codex_responses::{CodexProviderConfig, CodexResponsesProvider},
    openai::{OpenAiProvider, OpenAiProviderConfig},
};
pub use store::{
    ClaimRetrievalResult, ClaimStatus, ClaimSummary, ClaimWriteOutcome, EntityCandidate,
    EntityType, EvidenceAuthority, EvidenceCandidate, MemoryClaimDetail, MemoryClaimEvidence,
    MemoryClaimFilter, MemoryClaimRecord, NewClaimCandidate, NoemaStore, PredicateRecord,
    RetrievedClaim, StoreConfig, StoreError,
};
