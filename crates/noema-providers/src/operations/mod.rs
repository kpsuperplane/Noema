//! Object-safe provider generation operations.
//!
//! Concrete adapters implement [`ModelProvider`] with native futures. Runtime
//! consumers use [`ProviderHandle`] through this single erasure boundary.

use std::{collections::BTreeMap, fmt::Debug, future::Future, pin::Pin, sync::Arc};

use crate::generation::split_markdown_messages;
use crate::{
    DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest, GenerateResponse, GenerateResponseItem,
    GenerateStreamEvent, MarkdownMessageDeltaSplitter, ModelProvider, ProviderContextMetadata,
    ProviderError, ProviderResponseContinuation, ProviderSchemaCapabilities,
    ProviderToolCapabilities,
};

/// Boxed future returned by object-safe provider generation operations.
pub type ProviderOperationFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ProviderError>> + Send + 'a>>;

/// Boxed future returned by provider metadata lookups.
pub type ProviderContextFuture<'a> =
    Pin<Box<dyn Future<Output = ProviderContextMetadata> + Send + 'a>>;

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
    fn context_metadata<'a>(&'a self, _model: Option<&'a str>) -> ProviderContextFuture<'a> {
        Box::pin(async { ProviderContextMetadata::default() })
    }

    /// Return the provider's supported response-continuation strategy.
    fn response_continuation(&self, _model: Option<&str>) -> ProviderResponseContinuation {
        ProviderResponseContinuation::default()
    }

    /// Return native tool-calling capabilities for this provider/model.
    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities::default()
    }

    /// Return independent schema-enforcement capabilities for native tools and
    /// structured assistant output.
    fn schema_capabilities(&self, model: Option<&str>) -> ProviderSchemaCapabilities {
        self.tool_capabilities(model).schema_capabilities()
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
        Box::pin(async move {
            let mut response = ModelProvider::generate(&self.0, request).await?;
            response.normalize_markdown_messages();
            Ok(response)
        })
    }

    fn default_tool_classification_model(&self) -> Option<String> {
        ModelProvider::default_tool_classification_model(&self.0)
    }

    fn context_metadata<'a>(&'a self, model: Option<&'a str>) -> ProviderContextFuture<'a> {
        Box::pin(ModelProvider::context_metadata(&self.0, model))
    }

    fn response_continuation(&self, model: Option<&str>) -> ProviderResponseContinuation {
        ModelProvider::response_continuation(&self.0, model)
    }

    fn tool_capabilities(&self, model: Option<&str>) -> ProviderToolCapabilities {
        ModelProvider::tool_capabilities(&self.0, model)
    }

    fn schema_capabilities(&self, model: Option<&str>) -> ProviderSchemaCapabilities {
        ModelProvider::schema_capabilities(&self.0, model)
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
        Box::pin(async move {
            let mut splitters = BTreeMap::<usize, MarkdownMessageDeltaSplitter>::new();
            let mut response_indices = BTreeMap::<(usize, usize), usize>::new();
            let mut next_response_index = 0usize;
            let result = {
                let mut forward = |event| match event {
                    GenerateStreamEvent::AssistantTextDelta {
                        response_index,
                        delta,
                    } => {
                        let splitter = splitters.entry(response_index).or_default();
                        for (segment, delta) in splitter.push(&delta) {
                            let output_index = *response_indices
                                .entry((response_index, segment))
                                .or_insert_with(|| {
                                    let index = next_response_index;
                                    next_response_index = next_response_index.saturating_add(1);
                                    index
                                });
                            on_event(GenerateStreamEvent::AssistantTextDelta {
                                response_index: output_index,
                                delta,
                            });
                        }
                    }
                    event => on_event(event),
                };
                ModelProvider::generate_streaming(&self.0, request, &mut forward).await
            };
            for (source_index, splitter) in &mut splitters {
                for (segment, delta) in splitter.finish() {
                    let output_index = *response_indices
                        .entry((*source_index, segment))
                        .or_insert_with(|| {
                            let index = next_response_index;
                            next_response_index = next_response_index.saturating_add(1);
                            index
                        });
                    on_event(GenerateStreamEvent::AssistantTextDelta {
                        response_index: output_index,
                        delta,
                    });
                }
            }
            let mut response = result?;
            let mut normalized = Vec::new();
            for (source_index, item) in std::mem::take(&mut response.responses)
                .into_iter()
                .enumerate()
            {
                match item {
                    GenerateResponseItem::Text { phase, text } => {
                        for (segment, text) in
                            split_markdown_messages(&text).into_iter().enumerate()
                        {
                            let output_index = *response_indices
                                .entry((source_index, segment))
                                .or_insert_with(|| {
                                    let index = next_response_index;
                                    next_response_index = next_response_index.saturating_add(1);
                                    index
                                });
                            normalized
                                .push((output_index, GenerateResponseItem::Text { phase, text }));
                        }
                    }
                }
            }
            normalized.sort_by_key(|(output_index, _)| *output_index);
            response.responses = normalized.into_iter().map(|(_, item)| item).collect();
            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "adapters")]
    use crate::adapters::web::{default_web_fetch_backend, default_web_search_backend};
    use crate::{ProviderToolSchemaDialect, ProviderToolTransport};

    struct ContractProvider;

    #[derive(Debug)]
    struct InterleavedMessagesProvider;

    impl ModelProvider for InterleavedMessagesProvider {
        async fn generate(
            &self,
            _request: GenerateRequest,
        ) -> Result<GenerateResponse, ProviderError> {
            unreachable!("the test exercises streaming")
        }

        async fn generate_streaming<'a>(
            &'a self,
            _request: GenerateRequest,
            on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
        ) -> Result<GenerateResponse, ProviderError> {
            for (response_index, delta) in [(0, "first\n---\n"), (1, "second\n"), (0, "third")] {
                on_event(GenerateStreamEvent::AssistantTextDelta {
                    response_index,
                    delta: delta.to_string(),
                });
            }
            let mut response =
                GenerateResponse::final_text("first\n---\nthird", "interleaved", "model");
            response.responses.push(GenerateResponseItem::Text {
                phase: None,
                text: "second".to_string(),
            });
            Ok(response)
        }
    }

    impl Debug for ContractProvider {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("token=sentinel-secret path=/private/provider/session")
        }
    }

    impl ModelProvider for ContractProvider {
        async fn generate(
            &self,
            request: GenerateRequest,
        ) -> Result<GenerateResponse, ProviderError> {
            Ok(GenerateResponse::final_text(
                format!("generated:{}", request.input.render_for_token_count()),
                "contract",
                request.model.as_deref().unwrap_or("default"),
            ))
        }

        fn default_tool_classification_model(&self) -> Option<String> {
            Some("classification-model".to_string())
        }

        async fn context_metadata(&self, model: Option<&str>) -> ProviderContextMetadata {
            ProviderContextMetadata {
                context_window_tokens: (model == Some("large")).then_some(32_768),
                default_output_reserve_tokens: Some(1_024),
                compact_summary_target_tokens: Some(512),
            }
        }

        fn response_continuation(&self, _model: Option<&str>) -> ProviderResponseContinuation {
            ProviderResponseContinuation::PreviousResponseId {
                store_response: true,
            }
        }

        fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
            ProviderToolCapabilities {
                tool_transport: ProviderToolTransport::Native,
                parallel_tool_calls: true,
                tool_choice: true,
                schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
                native_tool_results: true,
                ..ProviderToolCapabilities::default()
            }
        }

        async fn count_tokens(
            &self,
            instructions: Option<&str>,
            input: &str,
            model: Option<&str>,
        ) -> Result<Option<u32>, ProviderError> {
            let bytes = instructions.map_or(0, str::len) + input.len() + model.map_or(0, str::len);
            Ok(Some(u32::try_from(bytes).expect("test length")))
        }

        async fn generate_streaming<'a>(
            &'a self,
            request: GenerateRequest,
            on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
        ) -> Result<GenerateResponse, ProviderError> {
            on_event(GenerateStreamEvent::AssistantTextDelta {
                response_index: 0,
                delta: "stream-delta".to_string(),
            });
            Ok(GenerateResponse::final_text(
                format!("streamed:{}", request.input.render_for_token_count()),
                "contract",
                request.model.as_deref().unwrap_or("default"),
            ))
        }
    }

    #[tokio::test]
    async fn provider_operation_boundaries_preserve_contract_defaults_and_redaction() {
        let provider = erase_model_provider(ContractProvider);
        assert_eq!(
            (
                provider.default_tool_classification_model(),
                provider.context_metadata(Some("large")).await,
                provider.response_continuation(None),
                provider.tool_capabilities(None),
            ),
            (
                Some("classification-model".to_string()),
                ProviderContextMetadata {
                    context_window_tokens: Some(32_768),
                    default_output_reserve_tokens: Some(1_024),
                    compact_summary_target_tokens: Some(512),
                },
                ProviderResponseContinuation::PreviousResponseId {
                    store_response: true,
                },
                ProviderToolCapabilities {
                    tool_transport: ProviderToolTransport::Native,
                    parallel_tool_calls: true,
                    tool_choice: true,
                    schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
                    native_tool_results: true,
                    ..ProviderToolCapabilities::default()
                },
            )
        );
        assert_eq!(
            provider
                .count_tokens(Some("rules"), "hello", Some("large"))
                .await
                .expect("tokens"),
            Some(15)
        );
        assert_eq!(
            provider
                .generate(GenerateRequest::text("plain").with_model("large"))
                .await
                .expect("generate")
                .assistant_text(),
            "generated:plain"
        );
        let mut events = Vec::new();
        let response = provider
            .generate_streaming(
                GenerateRequest::text("stream").with_model("large"),
                &mut |event| events.push(event),
            )
            .await
            .expect("stream");
        assert_eq!(response.assistant_text(), "streamed:stream");
        assert_eq!(
            events,
            vec![GenerateStreamEvent::AssistantTextDelta {
                response_index: 0,
                delta: "stream-delta".to_string(),
            }]
        );

        let debug = format!("{provider:?}");
        assert!(debug.contains("ErasedModelProvider"));
        assert!(!debug.contains("sentinel-secret"));
        assert!(!debug.contains("/private/provider/session"));

        #[derive(Debug)]
        struct DirectOperationsFake;

        impl ProviderOperations for DirectOperationsFake {
            fn generate_streaming<'a>(
                &'a self,
                request: GenerateRequest,
                _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
            ) -> ProviderOperationFuture<'a, GenerateResponse> {
                Box::pin(async move {
                    Ok(GenerateResponse::final_text(
                        request.input.render_for_token_count(),
                        "fake",
                        "fake-model",
                    ))
                })
            }
        }

        let provider: ProviderHandle = Arc::new(DirectOperationsFake);
        assert_eq!(
            (
                provider.default_tool_classification_model(),
                provider.context_metadata(None).await,
                provider.response_continuation(None),
                provider.tool_capabilities(None),
            ),
            (
                Some(DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string()),
                ProviderContextMetadata::default(),
                ProviderResponseContinuation::Unsupported,
                ProviderToolCapabilities::default(),
            )
        );
        assert_eq!(
            provider
                .count_tokens(Some("rules"), "input", Some("model"))
                .await
                .expect("default token count"),
            None
        );
        assert_eq!(
            provider
                .generate(GenerateRequest::text("generated"))
                .await
                .expect("default generation")
                .assistant_text(),
            "generated"
        );
        let mut events = Vec::new();
        assert_eq!(
            provider
                .generate_streaming(GenerateRequest::text("fallback"), &mut |event| events
                    .push(event),)
                .await
                .expect("streaming")
                .assistant_text(),
            "fallback"
        );
        assert!(events.is_empty());

        #[derive(Debug)]
        struct DefaultStreamingProvider;

        impl ModelProvider for DefaultStreamingProvider {
            async fn generate(
                &self,
                request: GenerateRequest,
            ) -> Result<GenerateResponse, ProviderError> {
                Ok(GenerateResponse::final_text(
                    request.input.render_for_token_count(),
                    "default-streaming",
                    "model",
                ))
            }
        }

        let mut events = Vec::new();
        let response = DefaultStreamingProvider
            .generate_streaming(GenerateRequest::text("hello stream"), &mut |event| {
                events.push(event)
            })
            .await
            .expect("default streaming");
        assert_eq!(response.assistant_text(), "hello stream");
        assert!(events.is_empty());

        #[cfg(feature = "adapters")]
        {
            assert_eq!(
                format!("{:?}", default_web_search_backend()),
                "WebSearchBackendHandle(\"[CONFIGURED]\")"
            );
            assert_eq!(
                format!("{:?}", default_web_fetch_backend()),
                "WebFetchBackendHandle(\"[CONFIGURED]\")"
            );
        }
    }

    #[tokio::test]
    async fn interleaved_source_messages_keep_distinct_stream_and_final_indices() {
        let provider = erase_model_provider(InterleavedMessagesProvider);
        let mut events = Vec::new();
        let response = provider
            .generate_streaming(GenerateRequest::text("ignored"), &mut |event| {
                events.push(event)
            })
            .await
            .expect("interleaved generation");

        assert_eq!(
            events,
            vec![
                GenerateStreamEvent::AssistantTextDelta {
                    response_index: 0,
                    delta: "first\n".to_string(),
                },
                GenerateStreamEvent::AssistantTextDelta {
                    response_index: 1,
                    delta: "second\n".to_string(),
                },
                GenerateStreamEvent::AssistantTextDelta {
                    response_index: 2,
                    delta: "third".to_string(),
                },
            ]
        );
        assert_eq!(
            response
                .responses
                .iter()
                .map(|item| match item {
                    GenerateResponseItem::Text { text, .. } => text.as_str(),
                })
                .collect::<Vec<_>>(),
            vec!["first", "second", "third"]
        );
    }
}
