//! Provider subsystem facade.
//!
//! Provider-neutral request/response contracts live in [`contract`]. Account
//! metadata and authentication support live alongside the contract, while
//! concrete provider adapters and transport helpers live under [`adapters`].

/// Neutral provider account metadata types.
pub mod accounts;
/// Concrete model provider adapters and transport helpers.
pub mod adapters;
/// Provider authentication support.
pub mod auth;
/// Provider-neutral generation request and response types.
pub mod contract;
/// Provider model/profile catalog refresh helpers.
pub mod model_catalog;

pub use contract::{
    AssistantTextPhase, DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateActionItem, GenerateInput,
    GenerateMessage, GenerateMessageRole, GenerateOptions, GenerateRequest, GenerateResponse,
    GenerateResponseItem, GenerateResponseStatus, GenerateStreamEvent, GenerateToolCall,
    ModelProvider, ParsedNoemaResponse, PromptCacheRetention, ProviderContextMetadata,
    ProviderError, TokenUsage, noema_response_from_text, output_items_from_text,
    required_noema_response_from_text,
};
