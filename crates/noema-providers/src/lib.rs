//! Provider-neutral contracts, configuration, and durable provider vocabulary.
//!
//! Concrete HTTP adapters and local inference implementations intentionally
//! live outside this crate. Its default feature set remains transport-free.

/// Provider account and authentication vocabulary.
pub mod accounts;
/// Provider capability declarations and assignments.
pub mod capabilities;
/// Resolved provider configuration.
pub mod config;
/// Provider generation contracts.
pub mod generation;
/// Durable local-model installation vocabulary.
pub mod local_models;
/// Provider model profile metadata.
pub mod model_profiles;
/// Provider-owned persistence boundaries.
pub mod persistence;
/// Immutable provider selection provenance.
pub mod selection;
/// Provider-neutral tool transport and selection policy.
pub mod tools;

pub use accounts::{
    CodexDeviceAuthRequest, NewProviderAccount, ProviderAccountCatalogEntry, ProviderAccountRecord,
    ProviderAccountStatus, ProviderAuthAttemptStatus, ProviderAuthAttemptView, ProviderAuthMethod,
    provider_account_catalog, system_provider_accounts,
};
pub use capabilities::{
    ProviderCapability, ProviderCapabilityAssignment, ProviderCapabilityStatus,
    capabilities_for_provider_account, provider_capability_assignment_pair_is_supported,
};
pub use config::{
    CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, CODEX_PROVIDER, CodexOAuthConfig, CodexOAuthTokens,
    CodexProviderConfig, DEFAULT_CODEX_BASE_URL, DEFAULT_CODEX_MODEL,
    DEFAULT_CODEX_OAUTH_CLIENT_ID, DEFAULT_CODEX_OAUTH_ISSUER, DEFAULT_CODEX_OAUTH_TIMEOUT_SECONDS,
    DEFAULT_CODEX_OAUTH_TOKEN_URL, DEFAULT_CODEX_TIMEOUT_SECONDS, DEFAULT_FOUNDATION_LOCAL_PROFILE,
    DEFAULT_LOCAL_MODELS_CONTEXT_WINDOW_TOKENS, DEFAULT_LOCAL_MODELS_PROFILE,
    DEFAULT_LOCAL_MODELS_STARTUP_TIMEOUT_SECONDS, DEFAULT_LOCAL_MODELS_TIMEOUT_SECONDS,
    DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_MODEL, DEFAULT_OPENAI_TIMEOUT_SECONDS,
    DEFAULT_PROVIDER, FoundationLocalProviderConfig, LocalModelsProviderConfig, OPENAI_API_KEY_ENV,
    OpenAiProviderConfig, ProviderConfig, ProviderKind,
};
pub use generation::{
    AssistantTextPhase, DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateActionItem, GenerateInput,
    GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateOptions,
    GenerateReasoningInput, GenerateReasoningItem, GenerateRequest, GenerateResponse,
    GenerateResponseItem, GenerateResponseStatus, GenerateStreamEvent, GenerateToolCall,
    GenerateToolCallInput, GenerateToolResultInput, GenerationPriority, ModelProvider,
    MultipleChoiceOption, MultipleChoiceSelectionMode, ParsedNoemaResponse, PromptCacheMode,
    PromptCacheOptions, PromptCacheRetention, PromptCacheTtl, ProviderContextMetadata,
    ProviderError, ProviderResponseContinuation, ProviderTransportContext, ProviderTransportKind,
    ReasoningEffort, TokenUsage, noema_response_from_text, output_items_from_text,
    required_noema_response_from_text, required_noema_response_from_text_with_native_tool_calls,
};
pub use local_models::{
    DefaultModelPreferenceRecord, LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelBackend,
    LocalModelCodecError, LocalModelEventKind, LocalModelEventRecord, LocalModelInstallationRecord,
    LocalModelInstallationStatus, LocalModelInstallationUpdate, LocalModelSourceKind,
    NewLocalModelInstallation, RemovedLocalModelInstallation,
};
pub use model_profiles::ProviderModelProfile;
pub use persistence::{
    LocalModelActivationPersistence, LocalModelActivationPersistenceHandle,
    LocalModelInstallationPersistence, LocalModelInstallationPersistenceHandle,
    PersistProviderModelCatalogRequest, ProviderAccountPersistence,
    ProviderAccountPersistenceHandle, ProviderAccountStatusUpdate,
    ProviderCapabilityAssignmentPersistence, ProviderCapabilityAssignmentPersistenceHandle,
    ProviderModelCatalogPersistence, ProviderModelCatalogPersistenceHandle,
    ProviderPersistenceError, ProviderPersistenceFuture, UpdateProviderAccountRequest,
    UpsertProviderCapabilityAssignmentRequest,
};
pub use selection::{
    ProviderInstanceKey, ProviderSelectionError, ProviderSelectionMode, ProviderSelectionSnapshot,
};
pub use tools::{
    NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, ProviderToolCapabilities,
    ProviderToolSchemaDialect, ProviderToolTransport,
};
