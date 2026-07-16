//! Object-safe provider generation operations.
//!
//! Concrete adapters implement [`ModelProvider`] with native futures. Runtime
//! consumers use [`ProviderHandle`] through this single erasure boundary.

use std::{fmt::Debug, future::Future, pin::Pin, sync::Arc};

use crate::{
    DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest, GenerateResponse, GenerateStreamEvent,
    ModelProvider, ProviderContextMetadata, ProviderError, ProviderResponseContinuation,
    ProviderToolCapabilities,
};

/// Boxed future returned by object-safe provider generation operations.
pub type ProviderOperationFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ProviderError>> + Send + 'a>>;

/// Object-safe generation operations consumed by runtimes and auxiliary model callers.
///
/// Cancellation remains caller-owned: dropping or selecting away from a
/// returned future cancels the caller's interest without introducing a second
/// provider cancellation contract.
pub trait ProviderOperations: Debug + Send + Sync {
    /// Generate a response for the given request.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] under the same conditions as
    /// [`ModelProvider::generate`].
    fn generate<'a>(
        &'a self,
        request: GenerateRequest,
    ) -> ProviderOperationFuture<'a, GenerateResponse> {
        Box::pin(async move {
            let mut ignore_event = |_| {};
            self.generate_streaming(request, &mut ignore_event).await
        })
    }

    /// Return the provider's preferred model for metadata-only tool classification.
    fn default_tool_classification_model(&self) -> Option<String> {
        Some(DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string())
    }

    /// Return provider/model context-window metadata used for prompt planning.
    fn context_metadata(&self, _model: Option<&str>) -> ProviderContextMetadata {
        ProviderContextMetadata::default()
    }

    /// Return the provider's supported response-continuation strategy.
    fn response_continuation(&self, _model: Option<&str>) -> ProviderResponseContinuation {
        ProviderResponseContinuation::default()
    }

    /// Return native tool-calling capabilities for this provider/model.
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities::default()
    }

    /// Count request tokens when the provider has an authoritative tokenizer.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] when provider token counting fails.
    fn count_tokens<'a>(
        &'a self,
        _instructions: Option<&'a str>,
        _input: &'a str,
        _model: Option<&'a str>,
    ) -> ProviderOperationFuture<'a, Option<u32>> {
        Box::pin(async { Ok(None) })
    }

    /// Generate a response while optionally emitting ephemeral stream events.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] under the same conditions as [`Self::generate`].
    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> ProviderOperationFuture<'a, GenerateResponse>;
}

/// Clonable object-safe provider generation handle.
pub type ProviderHandle = Arc<dyn ProviderOperations>;

/// Erase one typed [`ModelProvider`] behind the provider-owned operations boundary.
#[must_use]
pub fn erase_model_provider<T>(provider: T) -> ProviderHandle
where
    T: ModelProvider + Debug + 'static,
{
    Arc::new(ErasedModelProvider(provider))
}

struct ErasedModelProvider<T>(T);

impl<T> Debug for ErasedModelProvider<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ErasedModelProvider")
            .finish_non_exhaustive()
    }
}

impl<T> ProviderOperations for ErasedModelProvider<T>
where
    T: ModelProvider + Debug + 'static,
{
    fn generate<'a>(
        &'a self,
        request: GenerateRequest,
    ) -> ProviderOperationFuture<'a, GenerateResponse> {
        Box::pin(ModelProvider::generate(&self.0, request))
    }

    fn default_tool_classification_model(&self) -> Option<String> {
        ModelProvider::default_tool_classification_model(&self.0)
    }

    fn context_metadata(&self, model: Option<&str>) -> ProviderContextMetadata {
        ModelProvider::context_metadata(&self.0, model)
    }

    fn response_continuation(&self, model: Option<&str>) -> ProviderResponseContinuation {
        ModelProvider::response_continuation(&self.0, model)
    }

    fn tool_capabilities(&self, model: Option<&str>) -> ProviderToolCapabilities {
        ModelProvider::tool_capabilities(&self.0, model)
    }

    fn count_tokens<'a>(
        &'a self,
        instructions: Option<&'a str>,
        input: &'a str,
        model: Option<&'a str>,
    ) -> ProviderOperationFuture<'a, Option<u32>> {
        Box::pin(ModelProvider::count_tokens(
            &self.0,
            instructions,
            input,
            model,
        ))
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> ProviderOperationFuture<'a, GenerateResponse> {
        Box::pin(ModelProvider::generate_streaming(
            &self.0, request, on_event,
        ))
    }
}

#[cfg(test)]
mod tests;
