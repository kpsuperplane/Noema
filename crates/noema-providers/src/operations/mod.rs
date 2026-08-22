//! Object-safe provider generation operations.
//!
//! Concrete adapters implement [`ModelProvider`] with native futures. Runtime
//! consumers use [`ProviderHandle`] through this single erasure boundary.

use std::{collections::BTreeMap, fmt::Debug, future::Future, pin::Pin, sync::Arc};

use crate::generation::split_markdown_response_item;
use crate::{
    DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateRequest, GenerateResponse, GenerateStreamEvent,
    MarkdownMessageDeltaSplitter, ModelProvider, ProviderContextMetadata, ProviderError,
    ProviderGenerationFuture, ProviderGenerationMetadata, ProviderGenerationSession,
    ProviderResponseContinuation, ProviderSchemaRequestCapabilities, ProviderSessionInput,
    ProviderToolCapabilities,
};

#[cfg(test)]
use crate::GenerateResponseItem;

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

    /// Open one provider-owned session for a related generation sequence.
    fn open_generation_session(&self) -> Box<dyn ProviderGenerationSession + '_> {
        Box::new(OperationsGenerationSession { provider: self })
    }
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

const PROVIDER_CITATION_MARKER_START: &str = "\u{e200}cite\u{e202}";
const PROVIDER_CITATION_MARKER_END: char = '\u{e201}';

#[derive(Default)]
struct ProviderTextDeltaFilter {
    pending: String,
    inside_marker: bool,
}

impl ProviderTextDeltaFilter {
    fn push(&mut self, delta: &str) -> String {
        self.pending.push_str(delta);
        let mut visible = String::new();
        loop {
            if self.inside_marker {
                let boundary = self.pending.char_indices().find(|(_, character)| {
                    *character == PROVIDER_CITATION_MARKER_END || character.is_whitespace()
                });
                let Some((index, character)) = boundary else {
                    self.pending.clear();
                    break;
                };
                if character == PROVIDER_CITATION_MARKER_END {
                    self.pending
                        .drain(..index + PROVIDER_CITATION_MARKER_END.len_utf8());
                } else {
                    self.pending.drain(..index);
                }
                self.inside_marker = false;
                continue;
            }

            if let Some(index) = self.pending.find(PROVIDER_CITATION_MARKER_START) {
                visible.push_str(&self.pending[..index]);
                self.pending
                    .drain(..index + PROVIDER_CITATION_MARKER_START.len());
                self.inside_marker = true;
                continue;
            }

            let keep_from = self
                .pending
                .char_indices()
                .map(|(index, _)| index)
                .chain(std::iter::once(self.pending.len()))
                .find(|index| PROVIDER_CITATION_MARKER_START.starts_with(&self.pending[*index..]))
                .unwrap_or(self.pending.len());
            visible.push_str(&self.pending[..keep_from]);
            self.pending.drain(..keep_from);
            break;
        }
        visible
    }

    fn finish(&mut self) -> String {
        if self.inside_marker {
            self.pending.clear();
            String::new()
        } else {
            std::mem::take(&mut self.pending)
        }
    }
}

struct ErasedModelProvider<T>(T);

struct OperationsGenerationSession<'a, T>
where
    T: ProviderOperations + ?Sized,
{
    provider: &'a T,
}

impl<T> ProviderGenerationSession for OperationsGenerationSession<'_, T>
where
    T: ProviderOperations + ?Sized,
{
    fn generate<'a>(
        &'a mut self,
        mut request: GenerateRequest,
        input: ProviderSessionInput,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> ProviderGenerationFuture<'a> {
        request.input = input.replay;
        request.options.previous_response_id = None;
        request.options.store_response = false;
        self.provider.generate_streaming(request, on_event)
    }
}

struct NormalizedGenerationSession<'a> {
    inner: Box<dyn ProviderGenerationSession + 'a>,
}

impl ProviderGenerationSession for NormalizedGenerationSession<'_> {
    fn generate<'a>(
        &'a mut self,
        request: GenerateRequest,
        input: ProviderSessionInput,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> ProviderGenerationFuture<'a> {
        Box::pin(async move {
            let mut normalizer = GenerationNormalizer::new(on_event);
            let result = self
                .inner
                .generate(request, input, &mut |event| normalizer.push(event))
                .await;
            normalizer.finish(result)
        })
    }

    fn metadata(&self) -> ProviderGenerationMetadata {
        self.inner.metadata()
    }
}

struct GenerationNormalizer<'a> {
    on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    text_filters: BTreeMap<usize, ProviderTextDeltaFilter>,
    splitters: BTreeMap<usize, MarkdownMessageDeltaSplitter>,
    response_indices: BTreeMap<(usize, usize), usize>,
    next_response_index: usize,
}

impl<'a> GenerationNormalizer<'a> {
    fn new(on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send)) -> Self {
        Self {
            on_event,
            text_filters: BTreeMap::new(),
            splitters: BTreeMap::new(),
            response_indices: BTreeMap::new(),
            next_response_index: 0,
        }
    }

    fn output_index(&mut self, source_index: usize, segment: usize) -> usize {
        *self
            .response_indices
            .entry((source_index, segment))
            .or_insert_with(|| {
                let index = self.next_response_index;
                self.next_response_index = self.next_response_index.saturating_add(1);
                index
            })
    }

    fn push(&mut self, event: GenerateStreamEvent) {
        let GenerateStreamEvent::AssistantTextDelta {
            response_index,
            delta,
        } = event
        else {
            (self.on_event)(event);
            return;
        };
        let delta = self
            .text_filters
            .entry(response_index)
            .or_default()
            .push(&delta);
        let segments = self
            .splitters
            .entry(response_index)
            .or_default()
            .push(&delta);
        for (segment, delta) in segments {
            let response_index = self.output_index(response_index, segment);
            (self.on_event)(GenerateStreamEvent::AssistantTextDelta {
                response_index,
                delta,
            });
        }
    }

    fn finish(
        mut self,
        result: Result<GenerateResponse, ProviderError>,
    ) -> Result<GenerateResponse, ProviderError> {
        let source_indices = self.text_filters.keys().copied().collect::<Vec<_>>();
        for source_index in source_indices {
            let delta = self
                .text_filters
                .get_mut(&source_index)
                .expect("source index exists")
                .finish();
            if !delta.is_empty() {
                let segments = self.splitters.entry(source_index).or_default().push(&delta);
                for (segment, delta) in segments {
                    let response_index = self.output_index(source_index, segment);
                    (self.on_event)(GenerateStreamEvent::AssistantTextDelta {
                        response_index,
                        delta,
                    });
                }
            }
        }
        let source_indices = self.splitters.keys().copied().collect::<Vec<_>>();
        for source_index in source_indices {
            let segments = self
                .splitters
                .get_mut(&source_index)
                .expect("source index exists")
                .finish();
            for (segment, delta) in segments {
                let response_index = self.output_index(source_index, segment);
                (self.on_event)(GenerateStreamEvent::AssistantTextDelta {
                    response_index,
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
            for (segment, item) in split_markdown_response_item(item).into_iter().enumerate() {
                let output_index = self.output_index(source_index, segment);
                normalized.push((output_index, item));
            }
        }
        normalized.sort_by_key(|(output_index, _)| *output_index);
        response.responses = normalized.into_iter().map(|(_, item)| item).collect();
        Ok(response)
    }
}

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

    fn schema_request_capabilities(
        &self,
        model: Option<&str>,
    ) -> ProviderSchemaRequestCapabilities {
        ModelProvider::schema_request_capabilities(&self.0, model)
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
            let mut normalizer = GenerationNormalizer::new(on_event);
            let result = ModelProvider::generate_streaming(&self.0, request, &mut |event| {
                normalizer.push(event);
            })
            .await;
            normalizer.finish(result)
        })
    }

    fn open_generation_session(&self) -> Box<dyn ProviderGenerationSession + '_> {
        Box::new(NormalizedGenerationSession {
            inner: ModelProvider::open_generation_session(&self.0),
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

    #[derive(Debug)]
    struct CitationMarkerProvider;

    impl ModelProvider for CitationMarkerProvider {
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
            let raw = "Claim \u{e200}cite\u{e202}https://example.com/news\u{e201} done";
            for delta in [
                "Claim \u{e200}ci",
                "te\u{e202}https://example.com",
                "/news\u{e201} done",
            ] {
                on_event(GenerateStreamEvent::AssistantTextDelta {
                    response_index: 0,
                    delta: delta.to_string(),
                });
            }
            Ok(GenerateResponse::final_text(raw, "citation", "model"))
        }
    }

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
                id: None,
                phase: None,
                text: "second".to_string(),
                citations: Vec::new(),
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

        #[derive(Debug)]
        struct ExpiredActiveSessionProvider {
            inputs: Arc<std::sync::Mutex<Vec<crate::GenerateInput>>>,
        }

        impl ModelProvider for ExpiredActiveSessionProvider {
            async fn generate(
                &self,
                request: GenerateRequest,
            ) -> Result<GenerateResponse, ProviderError> {
                self.inputs
                    .lock()
                    .expect("inputs")
                    .push(request.input.clone());
                if matches!(request.input, crate::GenerateInput::NativeToolResults(_)) {
                    return Err(ProviderError::ProviderUnavailable {
                        provider: "active-session".to_string(),
                        message: "session expired".to_string(),
                    });
                }
                Ok(GenerateResponse::final_text(
                    "complete replay",
                    "active-session",
                    "model",
                ))
            }

            fn response_continuation(&self, _model: Option<&str>) -> ProviderResponseContinuation {
                ProviderResponseContinuation::ActiveSession
            }
        }

        let inputs = Arc::new(std::sync::Mutex::new(Vec::new()));
        let expired_provider = erase_model_provider(ExpiredActiveSessionProvider {
            inputs: Arc::clone(&inputs),
        });
        let mut session = expired_provider.open_generation_session();
        session
            .generate(
                GenerateRequest::text("initial"),
                ProviderSessionInput::initial(crate::GenerateInput::Text("initial".to_string())),
                &mut |_| {},
            )
            .await
            .expect("initial response");
        session
            .generate(
                GenerateRequest::text("complete replay"),
                ProviderSessionInput {
                    replay: crate::GenerateInput::Text("complete replay".to_string()),
                    incremental: Some(crate::GenerateInput::Items(vec![
                        crate::GenerateInputItem::ToolResult(crate::GenerateToolResultInput {
                            id: None,
                            call_id: "call-1".to_string(),
                            name: "search".to_string(),
                            provider_name: None,
                            arguments: serde_json::json!({}),
                            success: true,
                            payload: serde_json::json!({"result": "done"}),
                        }),
                    ])),
                },
                &mut |_| {},
            )
            .await
            .expect("expired session replay");
        let inputs = inputs.lock().expect("inputs");
        assert!(matches!(
            inputs[1],
            crate::GenerateInput::NativeToolResults(_)
        ));
        assert!(matches!(inputs[2], crate::GenerateInput::Text(_)));

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

    #[tokio::test]
    async fn citation_markers_are_hidden_from_streams_without_changing_provider_text() {
        let provider = erase_model_provider(CitationMarkerProvider);
        let mut visible = String::new();
        let response = provider
            .generate_streaming(GenerateRequest::text("ignored"), &mut |event| {
                if let GenerateStreamEvent::AssistantTextDelta { delta, .. } = event {
                    visible.push_str(&delta);
                }
            })
            .await
            .expect("citation generation");

        assert_eq!(visible, "Claim  done");
        assert_eq!(
            response.assistant_text(),
            "Claim \u{e200}cite\u{e202}https://example.com/news\u{e201} done"
        );
    }

    #[test]
    fn citation_filter_handles_every_delta_boundary_and_stream_termination() {
        let raw = "before \u{e200}cite\u{e202}turn0search0\u{e201} after";
        for split in raw
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(raw.len()))
        {
            let mut filter = ProviderTextDeltaFilter::default();
            let visible = format!(
                "{}{}{}",
                filter.push(&raw[..split]),
                filter.push(&raw[split..]),
                filter.finish()
            );
            assert_eq!(visible, "before  after", "split at byte {split}");
        }

        let mut incomplete = ProviderTextDeltaFilter::default();
        assert_eq!(
            format!(
                "{}{}",
                incomplete.push("before \u{e200}cite\u{e202}turn0search0"),
                incomplete.finish()
            ),
            "before "
        );
        let mut ordinary_prefix = ProviderTextDeltaFilter::default();
        assert_eq!(
            format!(
                "{}{}",
                ordinary_prefix.push("literal \u{e200}cit"),
                ordinary_prefix.finish()
            ),
            "literal \u{e200}cit"
        );
    }
}
