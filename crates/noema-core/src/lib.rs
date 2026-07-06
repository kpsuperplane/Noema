//! Core Noema types and runtimes.
//!
//! This crate contains configuration loading, provider adapters, daemon
//! runtime support, home-directory setup, path resolution, and the memory
//! retrieval model.

/// Capability Gateway runtime entrypoint.
pub mod capability;
/// Configuration loading and provider selection.
pub mod config;
/// Neutral conversation domain types.
pub mod conversation;
/// Local daemon runtime and web protocol types.
pub mod daemon;
/// GraphQL client API facade.
pub mod graphql;
/// Noema home-directory initialization.
pub mod home;
/// Neutral typed object ids.
pub mod ids;
/// Third-party MCP control-plane types.
pub mod mcp;
/// Memory extraction, consolidation, and graph retrieval policy model.
pub mod memory;
/// Neutral concrete object and actor references.
pub mod objects;
/// Onboarding status derived from provider account readiness.
pub mod onboarding;
/// Filesystem path resolution for Noema state.
pub mod paths;
/// Provider contracts, account/auth support, and concrete adapters.
pub mod provider;
/// Shared runtime host for daemon and desktop client surfaces.
pub mod runtime_host;
/// First-party governed web search capability.
pub mod search;
/// Embedded canonical structured store.
pub mod store;
/// Developer diagnostic system error logging.
pub mod system_errors;
#[doc(hidden)]
pub mod web_fetch;

pub use capability::{CapabilityGateway, GatewayToolProposal, GatewayToolResult};
pub use config::{
    Config, ConfigError, ConfigOverrides, DaemonResolvedConfig, FoundationLocalProviderConfig,
    ProviderConfig, ProviderKind, ResolvedConfig, WebConfig,
};
pub use conversation::{
    AgentStatus as PersistedAgentStatus, ConversationContextSummaryStatus, ConversationItemKind,
    ConversationItemPage, ConversationItemRecord, ConversationItemStatus, ConversationRecord,
    ConversationTurnRecord, ConversationTurnStatus, NewConversation, NewConversationItem,
    NewConversationTurn, ReplayMode,
};
pub use daemon::{
    AgentStatus, DaemonError, DaemonWebServerConfig, StartedConversation, TurnActivityStatus,
    TurnTranscriptItem, run_daemon_web,
};
pub use home::{
    DEFAULT_NOEMA_CONFIG_YAML, NoemaHomeError, NoemaHomeInitOptions, NoemaHomeInitResult,
    init_noema_home,
};
pub use ids::{ActorId, ContextPacketId, ConversationId, ConversationItemId, ObjectId};
pub use mcp::{
    McpCalibrationStatus, McpTransportKind, McpTrustClassification, OwnerExtractor,
    OwnerExtractorSource, TrustedIdentitySelectorKind, normalize_trusted_identity_value,
};
pub use memory::consolidation::{
    CanonicalClaimCandidate, CanonicalClaimStatus, CanonicalEntity, ConsolidationDecision,
    ConsolidationDecisionKind, MemoryConsolidationError, MemoryWriteProposal,
    MemoryWriteSourceKind, PredicateResolution, ProposedPredicate,
    build_claim_canonicalization_prompt, build_consolidation_prompt,
    parse_canonicalization_response, parse_consolidation_decision,
};
pub use memory::error::MemoryPersistenceError;
pub use memory::extraction::{
    ExtractorMemoryProposal, ExtractorMemoryResponse, MIN_MEMORY_EXTRACTION_CONFIDENCE,
    MemoryExtractionError, MemoryExtractionRetrievalHints, MemoryExtractionRiskFlag,
    MemoryExtractionSubject, MemoryExtractionSubjectKind, MemoryExtractionSubjectRole,
    ValidatedMemoryProposal, build_memory_extraction_prompt, decide_memory_proposal_status,
    parse_memory_extraction_proposals, validate_memory_extraction_response,
};
pub use memory::types::MemoryType;
pub use objects::{ActorKind, ActorRef, ObjectRef, ObjectType};
pub use onboarding::{
    OnboardingStatus, OnboardingStep, OnboardingStepStatus, onboarding_status_from_account,
};
pub use paths::{NOEMA_HOME_ENV, NoemaPathError, NoemaPaths};
pub use provider::accounts::{ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod};
pub use provider::adapters::{
    codex_responses::{CodexProviderConfig, CodexResponsesProvider},
    foundation_local::FoundationLocalProvider,
    openai::{OpenAiProvider, OpenAiProviderConfig},
};
pub use provider::{
    GenerateActionItem, GenerateInput, GenerateMessage, GenerateMessageRole, GenerateOptions,
    GenerateRequest, GenerateResponse, GenerateResponseItem, GenerateResponseStatus,
    GenerateToolCall, ModelProvider, PromptCacheRetention, ProviderContextMetadata, ProviderError,
    TokenUsage,
};
pub use runtime_host::{NoemaRuntimeHost, RuntimeHostError};
pub use store::{
    AgentRecord, AgentRuntimePreferenceRecord, AuxiliaryModelPreferenceRecord,
    ClaimRetrievalResult, ClaimStatus, ClaimSummary, ClaimWriteOutcome, ConsolidationMatch,
    ConsolidationMatchRequest, ConversationContextSummaryRecord, EntityCandidate, EntityType,
    EvidenceAuthority, EvidenceCandidate, McpApprovalRequestRecord, McpServerAuthStatus,
    McpServerHealthStatus, McpServerRecord, McpToolRecord, MemoryClaimDetail, MemoryClaimEvidence,
    MemoryClaimFilter, MemoryClaimRecord, MemoryGraph, MemoryGraphEdge, MemoryGraphFilter,
    MemoryGraphNode, MemoryGraphSummary, NewAgent, NewAgentRuntimePreference,
    NewAuxiliaryModelPreference, NewClaimCandidate, NewConversationContextSummary,
    NewMcpApprovalRequest, NewMcpServer, NewMcpTool, NewToolCalibration,
    NewTrustedIdentitySelector, NoemaStore, PredicateProposalCandidate, PredicateProposalFilter,
    PredicateProposalRecord, PredicateRecord, RelatedClaimCandidate, RelatedClaimRecord,
    RetrievedClaim, StoreConfig, StoreError, SupersedeClaimCandidate, ToolCalibrationRecord,
    TrustedIdentitySelectorEffect, TrustedIdentitySelectorRecord, WEB_FETCH_SUMMARIZER_TASK_ID,
};
pub use system_errors::{
    SYSTEM_ERROR_MCP_MALFORMED_RESPONSE, SYSTEM_ERROR_MCP_TOOL_CALL_FAILURE,
    SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, SYSTEM_ERROR_RUNTIME_INVARIANT,
    SYSTEM_ERROR_STORE_INVARIANT, SystemErrorEvent, SystemErrorLogger, SystemErrorWriteError,
};
