//! Provider subsystem facade.
//!
//! Provider-neutral request/response contracts live in [`contract`]. Account
//! metadata and authentication support live alongside the contract, while
//! concrete provider adapters and transport helpers live under [`adapters`].

/// Provider output or transport body was malformed.
pub const SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE: &str = "provider_malformed_response";

/// Neutral provider account metadata types.
pub mod accounts;
/// Concrete model provider adapters and transport helpers.
pub mod adapters;
/// Provider authentication support.
pub mod auth;
/// Provider capability declarations and behavior metadata.
pub mod capabilities;
/// Provider-neutral generation request and response types.
pub mod contract;
/// Provider model/profile catalog refresh helpers.
pub mod model_catalog;
/// Write-only secret-input provider account storage.
pub mod secret_input;
/// Provider-neutral native tool contracts.
pub mod tools;

pub use contract::{
    AssistantTextPhase, DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateActionItem, GenerateInput,
    GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateOptions,
    GenerateReasoningInput, GenerateReasoningItem, GenerateRequest, GenerateResponse,
    GenerateResponseItem, GenerateResponseStatus, GenerateStreamEvent, GenerateToolCall,
    GenerateToolCallInput, GenerateToolResultInput, GenerationPriority, ModelProvider,
    MultipleChoiceOption, MultipleChoiceSelectionMode, ParsedNoemaResponse, PromptCacheMode,
    PromptCacheOptions, PromptCacheRetention, PromptCacheTtl, ProviderContextMetadata,
    ProviderError, ProviderResponseContinuation, ReasoningEffort, TokenUsage,
    noema_response_from_text, output_items_from_text, required_noema_response_from_text,
    required_noema_response_from_text_with_native_tool_calls,
};

pub use capabilities::{
    CapabilityFeatures, CapabilityId, DataFlowClass, ProviderCapability, ProviderCapabilityStatus,
    ReliabilityContract, ResultPersistencePolicy, capabilities_for_provider_account,
};

pub use tools::{
    NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolCall, NoemaToolChoice, NoemaToolExecution,
    NoemaToolResult, NoemaToolSchema, NoemaToolSpec, ProviderToolCapabilities,
    ProviderToolSchemaDialect, ProviderToolTransport, ToolContractError, ToolName,
};
