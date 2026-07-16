use std::sync::Arc;

use super::*;
use crate::{ProviderToolSchemaDialect, ProviderToolTransport};

#[derive(Debug)]
struct ContractProvider;

impl ModelProvider for ContractProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        Ok(GenerateResponse::final_text(
            format!("generated:{}", request.input.render_for_token_count()),
            "contract",
            request.model.as_deref().unwrap_or("default"),
        ))
    }

    fn default_tool_classification_model(&self) -> Option<String> {
        Some("classification-model".to_string())
    }

    fn context_metadata(&self, model: Option<&str>) -> ProviderContextMetadata {
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
        Ok(Some(
            u32::try_from(bytes).expect("test input length fits u32"),
        ))
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
async fn erased_model_provider_preserves_the_complete_call_contract() {
    let provider = erase_model_provider(ContractProvider);
    let cloned: ProviderHandle = Arc::clone(&provider);

    assert_eq!(
        cloned.default_tool_classification_model().as_deref(),
        Some("classification-model")
    );
    assert_eq!(
        cloned.context_metadata(Some("large")),
        ProviderContextMetadata {
            context_window_tokens: Some(32_768),
            default_output_reserve_tokens: Some(1_024),
            compact_summary_target_tokens: Some(512),
        }
    );
    assert_eq!(
        cloned.response_continuation(Some("large")),
        ProviderResponseContinuation::PreviousResponseId {
            store_response: true
        }
    );
    assert_eq!(
        cloned.tool_capabilities(Some("large")),
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            parallel_tool_calls: true,
            tool_choice: true,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            native_tool_results: true,
            ..ProviderToolCapabilities::default()
        }
    );

    let instructions = "rules".to_string();
    let input = "hello".to_string();
    let model = "large".to_string();
    assert_eq!(
        cloned
            .count_tokens(Some(&instructions), &input, Some(&model))
            .await
            .expect("token count"),
        Some(15)
    );

    let generated = cloned
        .generate(GenerateRequest::text("plain").with_model("large"))
        .await
        .expect("generation");
    assert_eq!(generated.assistant_text(), "generated:plain");

    let mut events = Vec::new();
    let streamed = {
        let mut on_event = |event| events.push(event);
        cloned
            .generate_streaming(
                GenerateRequest::text("stream").with_model("large"),
                &mut on_event,
            )
            .await
            .expect("streaming generation")
    };
    assert_eq!(streamed.assistant_text(), "streamed:stream");
    assert_eq!(
        events,
        vec![GenerateStreamEvent::AssistantTextDelta {
            response_index: 0,
            delta: "stream-delta".to_string(),
        }]
    );
}

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

#[tokio::test]
async fn direct_operations_fakes_are_object_safe_and_keep_model_provider_defaults() {
    let provider: ProviderHandle = Arc::new(DirectOperationsFake);

    assert_eq!(
        provider.default_tool_classification_model().as_deref(),
        Some(DEFAULT_TOOL_CLASSIFICATION_MODEL)
    );
    assert_eq!(
        provider.context_metadata(None),
        ProviderContextMetadata::default()
    );
    assert_eq!(
        provider.response_continuation(None),
        ProviderResponseContinuation::Unsupported
    );
    assert_eq!(
        provider.tool_capabilities(None),
        ProviderToolCapabilities::default()
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
    let response = {
        let mut on_event = |event| events.push(event);
        provider
            .generate_streaming(GenerateRequest::text("fallback"), &mut on_event)
            .await
            .expect("default streaming fallback")
    };
    assert_eq!(response.assistant_text(), "fallback");
    assert!(events.is_empty());
}

struct SensitiveDebugProvider;

impl std::fmt::Debug for SensitiveDebugProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("token=sentinel-secret path=/private/provider/session")
    }
}

impl ModelProvider for SensitiveDebugProvider {
    async fn generate(&self, _request: GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        Ok(GenerateResponse::final_text(
            "redacted",
            "sensitive-debug-test",
            "test-model",
        ))
    }
}

#[test]
fn erased_provider_debug_never_delegates_to_the_concrete_provider() {
    let provider = erase_model_provider(SensitiveDebugProvider);

    let debug = format!("{provider:?}");

    assert!(debug.contains("ErasedModelProvider"));
    assert!(!debug.contains("sentinel-secret"));
    assert!(!debug.contains("/private/provider/session"));
}
