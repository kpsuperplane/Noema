//! Core Noema types and runtimes.
//!
//! This crate contains configuration loading, provider adapters, daemon
//! runtime support and the memory retrieval model. Filesystem layout and
//! developer diagnostics live in `noema-home`.

/// Shared execution roles and role-aware tool dispatch policy.
pub mod agent_execution;
/// Governed artifact filesystem helpers and local artifact writers.
pub mod artifacts;
/// Capability Gateway runtime entrypoint.
pub mod capability;
/// Configuration loading and provider selection.
pub mod config;
/// Local daemon runtime and web protocol types.
pub mod daemon;
/// GraphQL client API facade.
pub mod graphql;
/// Curated local-model catalog and hardware-fit recommendation.
pub mod local_models;
/// Third-party MCP control-plane types.
pub mod mcp;
/// Private OpenAI-compatible model proxy for local memory extraction.
pub mod memory_model_proxy;
/// Mnemosyne local service client and lifecycle support.
pub mod mnemosyne;
/// Onboarding status derived from provider account readiness.
pub mod onboarding;
/// Provider contracts, account/auth support, and concrete adapters.
pub mod provider;
/// Shared runtime host for daemon and desktop client surfaces.
pub mod runtime_host;
/// First-party governed web search capability.
pub mod search;
/// Embedded canonical structured store.
pub mod store;
/// Durable one-off task vocabulary and workflow contracts.
pub mod task;
#[doc(hidden)]
pub mod web_fetch;

pub use artifacts::{
    ArtifactWriteError, NewConversationLocalFileArtifact, NewTaskLocalFileArtifact,
    NewTaskLocalFileArtifactVersion, append_task_local_file_artifact_version,
    create_conversation_local_file_artifact, create_task_local_file_artifact,
};
pub use capability::{CapabilityGateway, GatewayToolProposal, GatewayToolResult};
pub use config::{
    Config, ConfigError, ConfigOverrides, DEFAULT_NOEMA_CONFIG_YAML, DaemonResolvedConfig,
    FoundationLocalProviderConfig, LocalModelsProviderConfig, ProviderConfig, ProviderKind,
    ResolvedConfig, WebConfig,
};
pub use daemon::{
    AgentStatus, DaemonError, StartedConversation, TurnActivityStatus, TurnTranscriptItem,
};
pub use graphql::RequestPrincipal;
pub use local_models::{
    HuggingFaceLocalModelImport, LLAMA_CPP_COMMIT, LLAMA_CPP_RELEASE_TAG, LLAMA_CPP_RUNTIME_ASSETS,
    LLAMA_SERVER_SIDECAR_BASENAME, LlamaCppRuntimeAsset, LlamaCppRuntimeAssetRole,
    LlamaServerCandidate, LlamaServerConfig, LlamaServerEndpoint, LlamaServerError,
    LlamaServerSupervisor, LocalFileModelImport, LocalHardwareProbeError, LocalHardwareProfile,
    LocalModelBackend, LocalModelBuild, LocalModelCatalog, LocalModelCatalogEntry,
    LocalModelCatalogError, LocalModelEventKind, LocalModelEventRecord, LocalModelInstallError,
    LocalModelInstallationRecord, LocalModelInstallationStatus, LocalModelInstallationUpdate,
    LocalModelInstaller, LocalModelRecommendation, LocalModelRuntimeStatus, LocalModelSourceKind,
    NOEMA_LLAMA_SERVER_PATH_ENV, NewLocalModelInstallation, RemovedLocalModelInstallation,
    bundled_llama_server_candidates, bundled_llama_server_candidates_in,
    detect_local_hardware_profiles, tauri_sidecar_input_name,
};
pub use mcp::{McpCalibrationStatus, McpTransportKind, McpTrustClassification};
pub use memory_model_proxy::{MemoryModelProxy, MemoryModelProxyConfig, MemoryModelProxyError};
pub use mnemosyne::{
    MnemosyneAddMemoryRequest, MnemosyneClient, MnemosyneClientError, MnemosyneConnection,
    MnemosyneLifecycle, MnemosyneLifecycleError, MnemosyneListMemoriesRequest,
    MnemosyneListMemoriesResponse, MnemosyneMemory, MnemosyneMessage, MnemosyneSearchRequest,
    MnemosyneSearchResponse,
};
pub use onboarding::{
    OnboardingStatus, OnboardingStep, OnboardingStepStatus, onboarding_status_from_account,
    onboarding_status_from_options,
};
pub use provider::accounts::{ProviderAccountRecord, ProviderAccountStatus, ProviderAuthMethod};
pub use provider::adapters::{
    codex_responses::{CodexProviderConfig, CodexResponsesProvider},
    foundation_local::FoundationLocalProvider,
    local_models::LocalModelsProvider,
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
    AgentRecord, AgentRunHeartbeat, AgentRunItemRecord, AgentRunItemStatus, AgentRunRecord,
    AgentRuntimePreferenceRecord, AgentSystemRole, AuxiliaryModelPreferenceRecord,
    McpServerAuthStatus, McpServerHealthStatus, McpServerRecord, McpToolRecord,
    MemoryArticleCacheRecord, MemoryServiceMode, MemoryServiceSettingsRecord, NewAgent,
    NewAgentRun, NewAgentRunItem, NewAgentRuntimePreference, NewAuxiliaryModelPreference,
    NewMcpServer, NewMcpTool, NewTaskModelPoolEntry, NewToolCalibration, NoemaStore,
    SaveMemoryArticleCache, SaveMemoryServiceSettings, StoreConfig, StoreError, TaskEventRecord,
    TaskModelPoolEntry, TaskRecord, TaskReviewRecord, TaskSubmissionArtifactRecord,
    TaskSubmissionRecord, ToolCalibrationRecord, WEB_FETCH_SUMMARIZER_TASK_ID,
};
pub use task::{
    CriterionOutcome, DEFAULT_TASK_MAX_ACTIVE_MINUTES, DEFAULT_TASK_MAX_PROVIDER_CONTINUATIONS,
    DEFAULT_TASK_MAX_REVIEW_ROUNDS, DEFAULT_TASK_MAX_TOOL_CALLS,
    DEFAULT_TASK_PROGRESS_AUDIT_INTERVAL, MAX_TASK_ACTIVE_MINUTES, MAX_TASK_PROVIDER_CONTINUATIONS,
    MAX_TASK_TOOL_CALLS, ModelConfigError, ModelConfigSnapshot, ModelSelectionMode, NewTask,
    NewTaskReview, NewTaskSubmission, NewTaskValidationCriterion, RunKind, RunStatus,
    SubmissionCriterionEvidence, TASK_EXECUTOR_AGENT_ID, TASK_REVIEWER_AGENT_ID, TaskComplexity,
    TaskDomainError, TaskExecutionPolicy, TaskReviewCriterion, TaskReviewVerdict, TaskSource,
    TaskStatus, TaskValidationCriterion,
};
