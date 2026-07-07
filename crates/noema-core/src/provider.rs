//! Provider subsystem facade.
//!
//! Provider-neutral request/response contracts live in [`contract`]. Account
//! metadata and authentication support live alongside the contract, while
//! concrete provider adapters and transport helpers live under [`adapters`].

/// Neutral provider account metadata types.
pub mod accounts;
/// Provider capability declarations and behavior metadata.
pub mod capabilities;
/// Concrete model provider adapters and transport helpers.
pub mod adapters;
/// Provider authentication support.
pub mod auth;
/// Provider-neutral generation request and response types.
pub mod contract;
/// Provider model/profile catalog refresh helpers.
pub mod model_catalog;
/// Provider-neutral native tool contracts.
pub mod tools;

pub use contract::{
    AssistantTextPhase, DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateActionItem, GenerateInput,
    GenerateInputItem, GenerateMessage, GenerateMessageRole, GenerateOptions,
    GenerateReasoningInput, GenerateReasoningItem, GenerateRequest, GenerateResponse,
    GenerateResponseItem, GenerateResponseStatus, GenerateStreamEvent, GenerateToolCall,
    GenerateToolCallInput, GenerateToolResultInput, ModelProvider, ParsedNoemaResponse,
    PromptCacheRetention, ProviderContextMetadata, ProviderError, TokenUsage,
    noema_response_from_text, output_items_from_text, required_noema_response_from_text,
    required_noema_response_from_text_with_native_tool_calls,
};

pub use capabilities::{
    CapabilityFeatures, CapabilityId, DataFlowClass, ProviderCapability, ProviderCapabilityStatus,
    ReliabilityContract, ResultPersistencePolicy, capabilities_for_provider_account,
};

pub use tools::{
    NoemaToolCall, NoemaToolChoice, NoemaToolExecution, NoemaToolResult, NoemaToolSchema,
    NoemaToolSpec, ProviderToolCapabilities, ProviderToolFallbackMode, ProviderToolSchemaDialect,
    ToolContractError, ToolExposurePolicy, ToolName,
};
