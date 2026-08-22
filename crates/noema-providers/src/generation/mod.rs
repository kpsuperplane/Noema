//! Provider-neutral generation contract.

mod error;
mod message_splitter;
mod request;
mod response;

use std::{future::Future, pin::Pin};

pub use error::{ProviderError, ProviderTransportContext, ProviderTransportKind};
pub(crate) use message_splitter::{MarkdownMessageDeltaSplitter, split_markdown_message_segments};
pub use request::{
    GenerateAssistantTextInput, GenerateInput, GenerateInputItem, GenerateMessage,
    GenerateMessageRole, GenerateOptions, GenerateReasoningInput, GenerateRequest,
    GenerateToolCallInput, GenerateToolResultInput, GenerationPriority, PromptCacheMode,
    PromptCacheOptions, PromptCacheRetention, PromptCacheTtl, ReasoningEffort,
};
pub(crate) use response::split_markdown_response_item;
pub use response::{
    AssistantResponseText, AssistantTextPhase, GenerateActionItem, GenerateCitation,
    GenerateHostedWebSearch, GenerateReasoningItem, GenerateResponse, GenerateResponseItem,
    GenerateStreamEvent, GenerateToolCall, GenerateWebSource, MultipleChoiceOption,
    MultipleChoiceSelectionMode, ProviderTimingMilestone, TokenUsage,
};

use crate::{ProviderSchemaRequestCapabilities, ProviderToolCapabilities};

/// Default model for small metadata classification tasks such as MCP tool hints.
pub const DEFAULT_TOOL_CLASSIFICATION_MODEL: &str = "gpt-5.4-mini";

// The provider contract stays a native async trait and does not expose
// `dyn ModelProvider`, so the public future-bound tradeoff is intentional.
/// A model backend that can produce structured output from a generation request.
pub trait ModelProvider: Send + Sync {
    /// Generate a response for the given request.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when the request is invalid, credentials are
    /// missing, the backend is unavailable, the backend returns an API error,
    /// or its response cannot be parsed.
    fn generate(
        &self,
        request: GenerateRequest,
    ) -> impl Future<Output = Result<GenerateResponse, ProviderError>> + Send;

    /// Return the provider's preferred model for metadata-only tool classification.
    fn default_tool_classification_model(&self) -> Option<String> {
        Some(DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string())
    }

    /// Return provider/model context-window metadata used for prompt planning.
    fn context_metadata(
        &self,
        _model: Option<&str>,
    ) -> impl Future<Output = ProviderContextMetadata> + Send {
        async { ProviderContextMetadata::default() }
    }

    /// Return the provider's supported response-continuation strategy.
    fn response_continuation(&self, _model: Option<&str>) -> ProviderResponseContinuation {
        ProviderResponseContinuation::default()
    }

    /// Return native tool-calling capabilities for this provider/model.
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities::default()
    }

    /// Return the schema request modes for native tools and structured output.
    fn schema_request_capabilities(
        &self,
        model: Option<&str>,
    ) -> ProviderSchemaRequestCapabilities {
        self.tool_capabilities(model).schema_request_capabilities()
    }

    /// Count request tokens when the provider has an authoritative tokenizer.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when provider token counting fails.
    fn count_tokens(
        &self,
        _instructions: Option<&str>,
        _input: &str,
        _model: Option<&str>,
    ) -> impl Future<Output = Result<Option<u32>, ProviderError>> + Send {
        async { Ok(None) }
    }

    /// Generate a response while optionally emitting ephemeral stream events.
    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> impl Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a {
        async move {
            let _ = on_event;
            self.generate(request).await
        }
    }

    /// Open one provider-owned session for a related generation sequence.
    fn open_generation_session(&self) -> Box<dyn ProviderGenerationSession + '_>
    where
        Self: Sized,
    {
        Box::new(DirectGenerationSession::new(self))
    }
}

/// Complete and optional changed input for one provider generation.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderSessionInput {
    /// Complete authoritative input for stateless replay.
    pub replay: GenerateInput,
    /// Input added after the preceding successful generation.
    pub incremental: Option<GenerateInput>,
}

impl ProviderSessionInput {
    /// Build the first input for a provider session.
    #[must_use]
    pub fn initial(replay: GenerateInput) -> Self {
        Self {
            replay,
            incremental: None,
        }
    }
}

/// Future returned by an object-safe provider generation session.
pub type ProviderGenerationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>>;

/// Provider-owned state for one related generation sequence.
pub trait ProviderGenerationSession: Send {
    /// Generate one response from authoritative replay and optional changed input.
    fn generate<'a>(
        &'a mut self,
        request: GenerateRequest,
        input: ProviderSessionInput,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> ProviderGenerationFuture<'a>;
}

struct DirectGenerationSession<'a, T> {
    provider: &'a T,
    previous_response_id: Option<String>,
    completed_generation: bool,
}

impl<'a, T> DirectGenerationSession<'a, T> {
    fn new(provider: &'a T) -> Self {
        Self {
            provider,
            previous_response_id: None,
            completed_generation: false,
        }
    }
}

impl<T> ProviderGenerationSession for DirectGenerationSession<'_, T>
where
    T: ModelProvider,
{
    fn generate<'a>(
        &'a mut self,
        mut request: GenerateRequest,
        input: ProviderSessionInput,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> ProviderGenerationFuture<'a> {
        Box::pin(async move {
            let strategy = self
                .provider
                .response_continuation(request.model.as_deref());
            request.options.previous_response_id = None;
            request.options.store_response = false;
            let can_continue = match strategy {
                ProviderResponseContinuation::Unsupported => false,
                ProviderResponseContinuation::PreviousResponseId { .. } => {
                    self.previous_response_id.is_some()
                }
                ProviderResponseContinuation::ActiveSession => self.completed_generation,
            };
            if let (true, Some(incremental)) = (can_continue, input.incremental) {
                request.input = incremental;
                request.options.previous_response_id = strategy
                    .supports_previous_response_id()
                    .then(|| self.previous_response_id.clone())
                    .flatten();
                request.options.store_response = strategy.store_response();
            } else {
                request.input = input.replay;
            }
            let response = self.provider.generate_streaming(request, on_event).await?;
            self.previous_response_id.clone_from(&response.response_id);
            self.completed_generation = true;
            Ok(response)
        })
    }
}

/// Provider/model context-window metadata for prompt planning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderContextMetadata {
    /// Maximum context window in tokens, if known.
    pub context_window_tokens: Option<u32>,
    /// Default output reserve for requests to this model.
    pub default_output_reserve_tokens: Option<u32>,
    /// Target summary size for compaction prompts.
    pub compact_summary_target_tokens: Option<u32>,
}

/// Provider strategy for continuing an earlier response without replaying its
/// full input over the wire.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProviderResponseContinuation {
    /// The provider requires complete stateless input replay.
    #[default]
    Unsupported,
    /// The provider accepts an opaque previous response identifier.
    PreviousResponseId {
        /// Whether the provider requires responses to be stored server-side
        /// before they can be referenced by a later request.
        store_response: bool,
    },
    /// The provider keeps the current native generation suspended locally and
    /// resumes it by receiving only the matching native tool results.
    ActiveSession,
}

impl ProviderResponseContinuation {
    /// Return whether this strategy can continue using a previous response id.
    #[must_use]
    pub const fn supports_previous_response_id(self) -> bool {
        matches!(self, Self::PreviousResponseId { .. })
    }

    /// Return whether this strategy resumes an in-process native generation.
    #[must_use]
    pub const fn supports_active_session(self) -> bool {
        matches!(self, Self::ActiveSession)
    }

    /// Return whether requests must ask the provider to retain their response.
    #[must_use]
    pub const fn store_response(self) -> bool {
        match self {
            Self::Unsupported | Self::ActiveSession => false,
            Self::PreviousResponseId { store_response } => store_response,
        }
    }
}
