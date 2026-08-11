//! Provider contracts, durable vocabulary, and optional concrete adapters.
//!
//! The default feature set remains transport-free. Hosted, Foundation, and web
//! adapters compile only with the `adapters` feature.

/// Concrete provider adapters and provider-owned integration services.
#[cfg(feature = "adapters")]
mod adapters;
#[cfg(feature = "adapters")]
pub(crate) mod chat_completions;

mod account_operations;
mod accounts;
mod capabilities;
mod config;
mod generation;
mod local_model;
/// First-party local GGUF provider implementation.
#[cfg(feature = "local-models")]
mod local_models;
mod model_preference;
mod model_profiles;
mod operations;
mod persistence;
mod recommendations;
mod registry;
/// Provider schema and diagnostic support shared by concrete adapters.
#[cfg(any(feature = "adapters", feature = "local-models"))]
mod response_support;
mod routing;
mod selection;
mod tools;
#[cfg(any(feature = "adapters", feature = "local-models"))]
mod transport_error;
mod web;

#[cfg(any(feature = "adapters", feature = "local-models"))]
pub(crate) use transport_error::reqwest_transport_error;

pub use account_operations::{
    CompleteProviderAuthCallbackRequest, CreateSecretProviderAccountRequest,
    ProviderAccountOperationError, ProviderAccountOperationFuture, ProviderAccountOperations,
    ProviderAccountOperationsHandle, ProviderAuthAttemptEventStream,
    SaveProviderAccountSecretRequest, StartProviderAuthRequest,
};
pub use accounts::{
    CodexDeviceAuthRequest, NewProviderAccount, ProviderAccountCatalogEntry, ProviderAccountRecord,
    ProviderAccountStatus, ProviderAuthAttemptStatus, ProviderAuthAttemptView, ProviderAuthMethod,
    provider_account_catalog, provider_account_instance_key,
};
#[cfg(feature = "adapters")]
pub use adapters::{
    EXA_FETCH_PROVIDER_ID, EXA_SEARCH_PROVIDER_ID, ExaFetchClient, ExaSearchClient,
    FoundationLocalProvider, OPENROUTER_PROVIDER_ACCOUNT_ID, ProviderAccountService,
    ProviderBootstrap, ProviderCredential, ProviderCredentialAccess,
    ProviderCredentialAccessHandle, ProviderCredentialFuture, default_web_browse_backend,
    default_web_fetch_backend, default_web_search_backend, hosted_provider_from_config,
    provider_bootstrap_from_config, summarize_markdown, validate_openrouter_api_key,
    web_fetch_summarizer_prompt,
};
pub use capabilities::{
    ProviderCapability, ProviderCapabilityAssignment, ProviderCapabilityStatus,
    capabilities_for_provider_account, provider_capability_assignment_pair_is_supported,
};
#[cfg(feature = "adapters")]
pub(crate) use config::{
    CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, CODEX_PROVIDER, DEFAULT_CODEX_MODEL,
};
pub use config::{
    CodexOAuthConfig, CodexOAuthTokens, CodexProviderConfig, DEFAULT_CODEX_BASE_URL,
    DEFAULT_CODEX_TIMEOUT_SECONDS, DEFAULT_FOUNDATION_LOCAL_PROFILE,
    DEFAULT_HOSTED_REASONING_EFFORT, DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS,
    DEFAULT_LOCAL_MODELS_PROFILE, DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS,
    DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS, DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_MODEL,
    DEFAULT_OPENAI_TIMEOUT_SECONDS, DEFAULT_OPENROUTER_BASE_URL, DEFAULT_OPENROUTER_MODEL,
    DEFAULT_OPENROUTER_TIMEOUT_SECONDS, DEFAULT_PROVIDER, FoundationLocalProviderConfig,
    LocalModelsProviderConfig, OPENAI_API_KEY_ENV, OpenAiProviderConfig, OpenRouterProviderConfig,
    ProviderConfig, ProviderKind,
};
pub(crate) use generation::MarkdownMessageDeltaSplitter;

/// Run the private browser worker when the process has its hidden worker arguments.
///
/// The caller must exit with the returned status instead of starting the normal application.
#[cfg(feature = "adapters")]
#[must_use]
pub fn run_browser_worker_if_requested() -> Option<i32> {
    adapters::run_worker_if_requested()
}
pub use generation::{
    AssistantTextPhase, DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateActionItem, GenerateCitation,
    GenerateHostedWebSearch, GenerateInput, GenerateInputItem, GenerateMessage,
    GenerateMessageRole, GenerateOptions, GenerateReasoningInput, GenerateReasoningItem,
    GenerateRequest, GenerateResponse, GenerateResponseItem, GenerateStreamEvent, GenerateToolCall,
    GenerateToolCallInput, GenerateToolResultInput, GenerateWebSource, GenerationPriority,
    ModelProvider, MultipleChoiceOption, MultipleChoiceSelectionMode, PromptCacheMode,
    PromptCacheOptions, PromptCacheRetention, PromptCacheTtl, ProviderContextMetadata,
    ProviderError, ProviderResponseContinuation, ProviderTransportContext, ProviderTransportKind,
    ReasoningEffort, TokenUsage,
};
pub use local_model::{
    DefaultModelPreferenceRecord, DegradedLocalModelInstance, HuggingFaceLocalModelImport,
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalFileModelImport, LocalHardwareProfile,
    LocalModelBackend, LocalModelBuild, LocalModelCatalogEntry, LocalModelCatalogSnapshot,
    LocalModelCatalogSnapshotEntry, LocalModelCodecError, LocalModelEventKind,
    LocalModelEventRecord, LocalModelInstallationRecord, LocalModelInstallationStatus,
    LocalModelInstallationUpdate, LocalModelManagement, LocalModelManagementFuture,
    LocalModelManager, LocalModelManagerConfig, LocalModelManagerError, LocalModelManagerEvent,
    LocalModelManagerEventRecord, LocalModelManagerEventStream, LocalModelReconstructionReport,
    LocalModelRuntimeStatus, LocalModelSourceKind, NewLocalModelInstallation,
    RemovedLocalModelInstallation, local_model_provider_instance_key,
};
#[cfg(feature = "local-model-evals")]
pub use local_models::{
    LocalModelEvalError, LocalModelEvalRuntimeVersion, LocalModelEvalSession,
    LocalModelEvalSessionConfig, MaterializeVerifiedEvalModelRequest, VerifiedEvalModelSource,
    local_model_eval_runtime_version, materialize_verified_eval_model,
};
pub use model_preference::ModelPreferenceSelection;
pub use model_profiles::ProviderModelProfile;
pub use operations::{
    ProviderContextFuture, ProviderHandle, ProviderOperationFuture, ProviderOperations,
    erase_model_provider,
};
pub use persistence::{
    ClaimedLocalModelInstallation, LocalModelActivationPersistence,
    LocalModelActivationPersistenceHandle, LocalModelInstallationPersistence,
    LocalModelInstallationPersistenceHandle, LocalModelInstanceReference,
    LocalModelInstanceReferenceSource, LocalModelLifecyclePersistence,
    LocalModelLifecyclePersistenceHandle, LocalModelReconstructionSnapshot,
    LocalModelRetirementClaimResult, LocalModelRuntimeRetirementResult,
    PersistProviderModelCatalogRequest, ProviderAccountPersistence,
    ProviderAccountPersistenceHandle, ProviderAccountStatusUpdate,
    ProviderCapabilityAccountReference, ProviderCapabilityAssignmentKey,
    ProviderCapabilityAssignmentPersistence, ProviderModelCatalogPersistence,
    ProviderModelCatalogPersistenceHandle, ProviderPersistenceError, ProviderPersistenceFuture,
    UpdateProviderAccountRequest, UpsertProviderCapabilityAssignmentRequest,
};
pub use recommendations::{
    NOEMA_MODEL_RECOMMENDATIONS, NoemaModelRecommendation, NoemaModelRecommendationCell,
    NoemaModelUseCase, noema_model_recommendation,
};
pub use registry::{
    ProviderInstanceLease, ProviderReadySelection, ProviderReadySelectionError,
    ProviderRegistration, ProviderRegistry, ProviderRegistryError, ProviderRegistryHandle,
    ProviderRetirementGuard,
};
pub use routing::{
    ProviderRouteError, ProviderRouteFuture, ProviderRouteLease, ProviderSelectionLoader,
    ProviderSelectionLoaderHandle, RegistryProviderRouteResolver, provider_selection_loader,
};
pub use selection::{
    ProviderInstanceKey, ProviderSelectionError, ProviderSelectionMode, ProviderSelectionSnapshot,
};
pub use tools::{
    NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, ProviderSchemaRequest,
    ProviderSchemaRequestCapabilities, ProviderTool, ProviderToolCapabilities,
    ProviderToolSchemaDialect, ProviderToolTransport, expose_provider_tools,
};
pub use web::{
    BEST_EFFORT_PUBLIC_CONTRACT, DIRECT_HTTP_PROVIDER_ID, DUCKDUCKGO_PUBLIC_PROVIDER_ID,
    EXTRACTION_READABILITYRS, OBSCURA_BROWSER_PROVIDER_ID, WebBrowseBackend,
    WebBrowseBackendHandle, WebBrowseError, WebBrowseOwner, WebFetchBackend, WebFetchBackendHandle,
    WebFetchContext, WebFetchError, WebOperationFuture, WebSearchBackend, WebSearchBackendHandle,
    WebSearchError,
};
