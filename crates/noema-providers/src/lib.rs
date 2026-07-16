//! Provider contracts, durable vocabulary, and optional concrete adapters.
//!
//! The default feature set remains transport-free. Hosted, Foundation, and web
//! adapters compile only with the `adapters` feature.

/// Concrete provider adapters and provider-owned integration services.
#[cfg(feature = "adapters")]
pub mod adapters;

/// Object-safe provider account orchestration contracts.
pub mod account_operations;
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
/// Object-safe provider generation operations.
pub mod operations;
/// Provider-owned persistence boundaries.
pub mod persistence;
/// Generation-safe provider instance registry.
pub mod registry;
/// Structured response support shared by hosted and local provider adapters.
#[cfg(any(feature = "adapters", feature = "local-models"))]
pub mod response_support;
/// Provider selection-to-instance route resolution.
pub mod routing;
/// Immutable provider selection provenance.
pub mod selection;
/// Provider-neutral tool transport and selection policy.
pub mod tools;
/// Provider-owned web backend contracts.
pub mod web;

pub use account_operations::{
    CreateSecretProviderAccountRequest, ProviderAccountOperationError,
    ProviderAccountOperationFuture, ProviderAccountOperations, ProviderAccountOperationsHandle,
    SaveProviderAccountSecretRequest, StartProviderAuthRequest,
};
pub use accounts::{
    CodexDeviceAuthRequest, NewProviderAccount, ProviderAccountCatalogEntry, ProviderAccountRecord,
    ProviderAccountStatus, ProviderAuthAttemptStatus, ProviderAuthAttemptView, ProviderAuthMethod,
    provider_account_catalog, system_provider_accounts,
};
#[cfg(feature = "adapters")]
pub use adapters::{
    CodexOAuthClient, CodexResponsesProvider, CodexTokenStore, DirectHttpClient,
    DuckDuckGoSearchBackend, EXA_EXTRACTION, EXA_FETCH_PROVIDER_ID, EXA_SEARCH_CONTRACT,
    EXA_SEARCH_PROVIDER_ID, ExaFetchClient, ExaSearchClient,
    FOUNDATION_LOCAL_COMPACT_SUMMARY_TARGET_TOKENS, FOUNDATION_LOCAL_CONTEXT_WINDOW_TOKENS,
    FOUNDATION_LOCAL_DEFAULT_OUTPUT_RESERVE_TOKENS, FOUNDATION_LOCAL_PROVIDER,
    FoundationBridgeError, FoundationLocalProvider, OpenAiProvider, ProviderAccountService,
    ProviderAuthManager, ProviderCredential, ProviderCredentialAccess,
    ProviderCredentialAccessHandle, ProviderCredentialFuture, SecretInputStore,
    default_web_fetch_backend, default_web_search_backend, ensure_provider_account_home,
    refresh_provider_model_profiles, summarize_markdown, web_fetch_summarizer_prompt,
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
pub use operations::{
    ProviderHandle, ProviderOperationFuture, ProviderOperations, erase_model_provider,
};
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
pub use registry::{
    ProviderInstanceLease, ProviderRegistration, ProviderRegistry, ProviderRegistryError,
    ProviderRegistryHandle, ProviderRetirementGuard,
};
pub use routing::{
    ProviderRouteError, ProviderRouteFuture, ProviderRouteLease, ProviderRouteResolver,
    ProviderRouteResolverHandle, ProviderSelectionLoader, ProviderSelectionLoaderHandle,
    RegistryProviderRouteResolver, provider_selection_loader,
};
pub use selection::{
    ProviderInstanceKey, ProviderSelectionError, ProviderSelectionMode, ProviderSelectionSnapshot,
};
pub use tools::{
    NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, ProviderToolCapabilities,
    ProviderToolSchemaDialect, ProviderToolTransport,
};
pub use web::{
    BEST_EFFORT_PUBLIC_CONTRACT, DIRECT_HTTP_PROVIDER_ID, DUCKDUCKGO_PUBLIC_PROVIDER_ID,
    EXTRACTION_READABILITYRS, WebFetchBackend, WebFetchBackendHandle, WebFetchContext,
    WebFetchError, WebOperationFuture, WebSearchBackend, WebSearchBackendHandle, WebSearchError,
};
