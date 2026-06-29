//! Core Noema types and runtimes.
//!
//! This crate contains configuration loading, provider adapters, daemon
//! protocol support, home-directory setup, path resolution, and the memory
//! retrieval model.

/// Configuration loading and provider selection.
pub mod config;
/// Neutral conversation domain types.
pub mod conversation;
/// Local daemon protocol and client/server runtime.
pub mod daemon;
/// GraphQL client API facade.
pub mod graphql;
/// Noema home-directory initialization.
pub mod home;
/// Neutral typed object ids.
pub mod ids;
/// Memory storage and retrieval policy model.
pub mod memory;
/// Future-write memory consolidation types and prompts.
pub mod memory_consolidation;
/// Shared memory/domain error types.
pub mod memory_error;
/// Pure ordinary-chat memory extraction proposal layer.
pub mod memory_extraction;
/// Neutral memory classification and candidate types.
pub mod memory_types;
/// Neutral concrete object and actor references.
pub mod objects;
/// Onboarding status derived from provider account readiness.
pub mod onboarding;
/// Filesystem path resolution for Noema state.
pub mod paths;
/// Provider-neutral generation request and response types.
pub mod provider;
/// Neutral provider account metadata types.
pub mod provider_accounts;
/// Provider authentication support.
pub mod provider_auth;
/// Concrete model provider adapters.
pub mod providers;
/// Embedded canonical structured store.
pub mod store;

pub use config::{
    CliOverrides, Config, ConfigError, DaemonResolvedConfig, ProviderConfig, ProviderKind,
    ResolvedConfig, WebConfig,
};
pub use conversation::{
    AgentStatus as PersistedAgentStatus, ConversationItemKind, ConversationItemRecord,
    ConversationItemStatus, ConversationRecord, ConversationTurnRecord, ConversationTurnStatus,
    NewConversation, NewConversationItem, NewConversationTurn, ReplayMode,
};
pub use daemon::{
    AgentStatus, DaemonClient, DaemonError, DaemonRequest, DaemonResponse, DaemonServerConfig,
    StartedConversation, TurnActivityStatus, TurnTranscriptItem, default_socket_path,
    is_connection_refused, run_daemon, socket_path_for_home,
};
pub use home::{
    DEFAULT_NOEMA_CONFIG_YAML, NoemaHomeError, NoemaHomeInitOptions, NoemaHomeInitResult,
    init_noema_home,
};
pub use ids::{
    ActorId, ContextPacketId, ConversationId, ConversationItemId, MemoryItemId, ObjectId,
};
pub use memory_consolidation::{
    CanonicalClaimCandidate, CanonicalClaimStatus, CanonicalEntity, ConsolidationDecision,
    ConsolidationDecisionKind, MemoryConsolidationError, MemoryWriteProposal,
    MemoryWriteSourceKind, PredicateResolution, ProposedPredicate,
    build_claim_canonicalization_prompt, build_consolidation_prompt,
    parse_canonicalization_response, parse_consolidation_decision,
};
pub use memory_error::MemoryPersistenceError;
pub use memory_extraction::{
    ExtractorMemoryProposal, ExtractorMemoryResponse, MIN_MEMORY_EXTRACTION_CONFIDENCE,
    MemoryExtractionError, MemoryExtractionRetrievalHints, MemoryExtractionRiskFlag,
    MemoryExtractionSubject, MemoryExtractionSubjectKind, MemoryExtractionSubjectRole,
    ValidatedMemoryProposal, build_memory_extraction_prompt, decide_memory_proposal_status,
    parse_memory_extraction_proposals, validate_memory_extraction_response,
};
pub use memory_types::{
    MemoryAuthorityLevel, MemoryExtractionMethod, MemorySummary, MemoryType, NewMemoryCandidate,
    NewMemoryParticipant, NewMemorySubject, ObjectProvenanceSource,
};
pub use objects::{ActorKind, ActorRef, ObjectRef, ObjectType};
pub use onboarding::{
    OnboardingStatus, OnboardingStep, OnboardingStepStatus, onboarding_status_from_account,
};
pub use paths::{NOEMA_HOME_ENV, NoemaPathError, NoemaPaths};
pub use provider::{
    GenerateInput, GenerateOptions, GenerateOutputItem, GenerateRequest, GenerateResponse,
    ModelProvider, ProviderError, TokenUsage,
};
pub use provider_accounts::{
    ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod, parse_account_status,
    parse_auth_method,
};
pub use providers::{
    codex_responses::{CodexProviderConfig, CodexResponsesProvider},
    openai::{OpenAiProvider, OpenAiProviderConfig},
};
pub use store::{
    ClaimRetrievalResult, ClaimStatus, ClaimSummary, ClaimWriteOutcome, EntityCandidate,
    EntityType, EvidenceAuthority, EvidenceCandidate, MemoryClaimDetail, MemoryClaimEvidence,
    MemoryClaimFilter, MemoryClaimRecord, MemoryGraph, MemoryGraphEdge, MemoryGraphFilter,
    MemoryGraphNode, MemoryGraphSummary, NewClaimCandidate, NoemaStore, PredicateRecord,
    RetrievedClaim, StoreConfig, StoreError,
};
