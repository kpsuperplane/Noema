//! Core Noema types and runtimes.
//!
//! This crate contains configuration loading, provider adapters, daemon
//! runtime support, home-directory setup, path resolution, and the memory
//! retrieval model.

/// Shared execution roles and role-aware tool dispatch policy.
pub mod agent_execution;
/// Governed artifact filesystem helpers and local artifact writers.
pub mod artifacts;
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
/// Shared memory persistence error types.
pub mod memory;
/// Private OpenAI-compatible model proxy for local memory extraction.
pub mod memory_model_proxy;
/// Mnemosyne local service client and lifecycle support.
pub mod mnemosyne;
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
/// Durable one-off task vocabulary and workflow contracts.
pub mod task;
#[doc(hidden)]
pub mod web_fetch;

pub use artifacts::{
    ArtifactWriteError, NewConversationLocalFileArtifact, artifact_download_url,
    artifact_version_id_from_download_slug, create_conversation_local_file_artifact,
};
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
    AgentStatus, DaemonError, StartedConversation, TurnActivityStatus, TurnTranscriptItem,
};
pub use graphql::RequestPrincipal;
pub use home::{
    DEFAULT_NOEMA_CONFIG_YAML, NoemaHomeError, NoemaHomeInitOptions, NoemaHomeInitResult,
    init_noema_home,
};
pub use ids::{
    ActorId, AgentRunId, ContextPacketId, ConversationId, ConversationItemId, ObjectId,
    TaskEventId, TaskId, TaskReviewId, TaskSubmissionId,
};
pub use mcp::{McpCalibrationStatus, McpTransportKind, McpTrustClassification};
pub use memory::error::MemoryPersistenceError;
pub use memory_model_proxy::{MemoryModelProxy, MemoryModelProxyConfig, MemoryModelProxyError};
pub use mnemosyne::{
    MnemosyneAddMemoryRequest, MnemosyneClient, MnemosyneClientError, MnemosyneConnection,
    MnemosyneLifecycle, MnemosyneLifecycleError, MnemosyneListMemoriesRequest,
    MnemosyneListMemoriesResponse, MnemosyneMemory, MnemosyneMessage, MnemosyneSearchRequest,
    MnemosyneSearchResponse,
};
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
    AgentRecord, AgentRunRecord, AgentRuntimePreferenceRecord, AgentSystemRole, ArtifactOwnerRef,
    ArtifactRecord, ArtifactSource, ArtifactStorageKind, ArtifactVersionRecord,
    ArtifactVersionStorage, ArtifactWithVersions, AuxiliaryModelPreferenceRecord,
    ConversationContextSummaryRecord, McpServerAuthStatus, McpServerHealthStatus, McpServerRecord,
    McpToolRecord, MemoryArticleCacheRecord, MemoryServiceMode, MemoryServiceSettingsRecord,
    NewAgent, NewAgentRun, NewAgentRuntimePreference, NewArtifact, NewArtifactVersion,
    NewAuxiliaryModelPreference, NewConversationContextSummary, NewMcpServer, NewMcpTool,
    NewTaskModelPoolEntry, NewToolCalibration, NoemaStore, SaveMemoryArticleCache,
    SaveMemoryServiceSettings, StoreConfig, StoreError, TaskModelPoolEntry, TaskRecord,
    ToolCalibrationRecord, WEB_FETCH_SUMMARIZER_TASK_ID, validate_external_artifact_url,
};
pub use system_errors::{
    SYSTEM_ERROR_MCP_MALFORMED_RESPONSE, SYSTEM_ERROR_MCP_TOOL_CALL_FAILURE,
    SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, SYSTEM_ERROR_RUNTIME_INVARIANT,
    SYSTEM_ERROR_STORE_INVARIANT, SystemErrorEvent, SystemErrorLogger, SystemErrorWriteError,
};
pub use task::{
    CriterionOutcome, DEFAULT_TASK_MAX_REVIEW_ROUNDS, ModelConfigError, ModelConfigSnapshot,
    ModelSelectionMode, NewTask, NewTaskReview, NewTaskSubmission, NewTaskValidationCriterion,
    RunKind, RunStatus, SubmissionCriterionEvidence, TASK_EXECUTOR_AGENT_ID,
    TASK_REVIEWER_AGENT_ID, TaskComplexity, TaskDomainError, TaskReviewCriterion,
    TaskReviewVerdict, TaskSource, TaskStatus, TaskValidationCriterion,
};
