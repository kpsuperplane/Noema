use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use noema_store::{NewAuxiliaryModelPreference, WEB_FETCH_SUMMARIZER_TASK_ID};

use crate::daemon::{
    agent_onboarding::AgentPromptIdentity,
    runtime::{
        actor::RuntimeActor, model_tools::ModelTools, tool_lifecycle::LocalToolCall,
        turn::SuccessfulProviderTurn,
    },
};
use noema_capabilities::{CapabilityError, CapabilityFuture, CapabilityInvoker, CapabilityOutput};
use noema_providers::{
    GenerateActionItem, GenerateInput, GenerateRequest, GenerateResponse, GenerateResponseItem,
    GenerateResponseStatus, GenerateStreamEvent, ProviderCapabilityAccountReference, ProviderError,
    ProviderOperations, ProviderToolCapabilities,
};
use serde_json::{Value, json};

async fn upsert_ready_auxiliary_model_preference(
    store: &noema_store::NoemaStore,
    preference: NewAuxiliaryModelPreference,
) {
    let ready_selection = crate::test_support::ready_provider_selection(
        noema_providers::ProviderSelectionSnapshot::explicit(
            &preference.provider_kind,
            &preference.provider_account_id,
            &preference.model_profile,
            preference.reasoning_effort,
            Some(format!("auxiliary_model_preference:{}", preference.task_id)),
        ),
    );
    store
        .upsert_auxiliary_model_preference_with_ready_selection(preference, &ready_selection)
        .await
        .expect("ready auxiliary model preference");
}

#[derive(Debug)]
struct LocalToolTestProvider {
    default_tool_model: Option<String>,
    requests: Arc<Mutex<Vec<GenerateRequest>>>,
}

struct RecordingCapabilityInvoker {
    invocations: Mutex<Vec<noema_capabilities::CapabilityInvocation>>,
    result: Result<CapabilityOutput, CapabilityError>,
}

impl RecordingCapabilityInvoker {
    fn returning(result: Result<CapabilityOutput, CapabilityError>) -> Self {
        Self {
            invocations: Mutex::new(Vec::new()),
            result,
        }
    }
}

impl Default for RecordingCapabilityInvoker {
    fn default() -> Self {
        Self::returning(Ok(CapabilityOutput::success(
            json!({"document": "contents"}),
        )))
    }
}

impl CapabilityInvoker for RecordingCapabilityInvoker {
    fn invoke(
        &self,
        invocation: noema_capabilities::CapabilityInvocation,
    ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>> {
        self.invocations
            .lock()
            .expect("invocation lock")
            .push(invocation);
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

impl ProviderOperations for LocalToolTestProvider {
    fn default_tool_classification_model(&self) -> Option<String> {
        self.default_tool_model.clone()
    }

    fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
        ProviderToolCapabilities::default()
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            self.requests
                .lock()
                .expect("requests")
                .push(request.clone());
            Ok(GenerateResponse {
                responses: vec![GenerateResponseItem::Text {
                    phase: None,
                    text: "summarized page".to_string(),
                }],
                tool_calls: Vec::new(),
                reasoning_items: Vec::new(),
                response_status: GenerateResponseStatus::Final,
                provider: "test".to_string(),
                model: request
                    .model
                    .unwrap_or_else(|| "missing-test-model".to_string()),
                response_id: None,
                usage: None,
            })
        })
    }
}

async fn test_actor() -> RuntimeActor {
    let store = crate::test_support::test_store().await;
    RuntimeActor::new(
        "codex".to_string(),
        HashMap::from([(
            "codex".to_string(),
            Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                as noema_providers::ProviderHandle,
        )]),
        store.clone(),
        crate::test_support::system_error_logger(),
    )
    .await
    .expect("actor")
}

impl LocalToolTestProvider {
    fn new(default_tool_model: Option<&str>) -> Self {
        Self {
            default_tool_model: default_tool_model.map(str::to_string),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_requests(
        default_tool_model: Option<&str>,
        requests: Arc<Mutex<Vec<GenerateRequest>>>,
    ) -> Self {
        Self {
            default_tool_model: default_tool_model.map(str::to_string),
            requests,
        }
    }
}

fn test_turn() -> SuccessfulProviderTurn {
    test_turn_with_selection(noema_providers::ProviderSelectionSnapshot::explicit(
        "codex",
        "provider_account:codex:default",
        "gpt-test",
        None,
        Some("test".to_string()),
    ))
}

fn test_turn_with_selection(
    selection: noema_providers::ProviderSelectionSnapshot,
) -> SuccessfulProviderTurn {
    let initial_model_tools = test_web_model_tools();
    let provider_kind = selection.provider_kind.clone();
    let model = selection.model_profile.clone();
    let reasoning_effort = selection.reasoning_effort;
    SuccessfulProviderTurn {
        conversation_id: "conversation:test".to_string(),
        turn_id: "turn:test".to_string(),
        turn_index: 1,
        user_item_id: "item:user:test".to_string(),
        user_input: "test".to_string(),
        task_id: None,
        task_run_id: None,
        cwd: None,
        provider_kind: provider_kind.clone(),
        model: model.clone(),
        reasoning_effort,
        provider_route: crate::test_support::provider_route(
            selection,
            Arc::new(LocalToolTestProvider::new(Some("codex-tool-default"))),
        ),
        initial_stream_id: "stream:test".to_string(),
        response: GenerateResponse {
            responses: Vec::new(),
            tool_calls: Vec::new(),
            reasoning_items: Vec::new(),
            response_status: GenerateResponseStatus::Final,
            provider: provider_kind,
            model: model.unwrap_or_else(|| "provider-default".to_string()),
            response_id: None,
            usage: None,
        },
        agent_identity: AgentPromptIdentity {
            agent_id: "agent:primary".to_string(),
            display_name: None,
        },
        runtime_environment: crate::daemon::runtime::turn::current_runtime_environment(None),
        tool_capabilities: ProviderToolCapabilities::default(),
        initial_model_tools: initial_model_tools.clone(),
        continuation_model_tools: initial_model_tools,
        initial_provider_input: GenerateInput::Text("test".to_string()),
    }
}

fn test_web_model_tools() -> ModelTools {
    let mut builder = noema_capabilities::CapabilityCatalogBuilder::new();
    let mut policy = crate::agent_execution::ToolPolicy::default();
    let mut prompt_kinds = std::collections::BTreeMap::new();
    for spec in [
        noema_capabilities::web::search::tool_spec().expect("search spec"),
        noema_capabilities::web::fetch::tool_spec().expect("fetch spec"),
    ] {
        let name = spec.name.as_str().to_string();
        let sanitizer: Arc<dyn noema_capabilities::PayloadSanitizer> =
            if name == noema_capabilities::web::fetch::WEB_FETCH_TOOL {
                Arc::new(noema_capabilities::WebFetchPayloadSanitizer)
            } else {
                Arc::new(noema_capabilities::RedactingPayloadSanitizer)
            };
        builder
            .add(noema_capabilities::CapabilityBinding::new(
                spec,
                noema_capabilities::CapabilityTarget::new(
                    noema_capabilities::InvokerKey::new("runtime-execution"),
                    noema_capabilities::OperationToken::new(name.clone()),
                ),
                noema_capabilities::CapabilityAccess {
                    effect: noema_capabilities::CapabilityEffect::ReadOnly,
                    scope: noema_capabilities::CapabilityScope::Global,
                },
                sanitizer,
            ))
            .expect("unique binding");
        policy.allow_tool_name(name.clone());
        prompt_kinds.insert(
            name,
            crate::daemon::runtime::model_tools::ModelToolPromptKind::Web,
        );
    }
    ModelTools {
        transport: noema_providers::ProviderToolTransport::Native,
        bindings: builder.build(),
        prompt_rows: Vec::new(),
        unavailable_rows: Vec::new(),
        prompt_kinds,
        tool_policy: policy,
    }
}

const TEST_CAPABILITY_NAME: &str = "extension.docs.read";

fn test_injected_capability_model_tools(
    sanitizer: Arc<dyn noema_capabilities::PayloadSanitizer>,
) -> ModelTools {
    let spec = noema_capabilities::ToolSpec::new(
        TEST_CAPABILITY_NAME,
        "Read a document.",
        json!({"type": "object"}),
    )
    .expect("tool spec");
    let mut builder = noema_capabilities::CapabilityCatalogBuilder::new();
    builder
        .add(noema_capabilities::CapabilityBinding::new(
            spec,
            noema_capabilities::CapabilityTarget::new(
                noema_capabilities::InvokerKey::new("external:test"),
                noema_capabilities::OperationToken::new("opaque-child-authority"),
            ),
            noema_capabilities::CapabilityAccess {
                effect: noema_capabilities::CapabilityEffect::ReadOnly,
                scope: noema_capabilities::CapabilityScope::Global,
            },
            sanitizer,
        ))
        .expect("unique binding");
    let mut policy = crate::agent_execution::ToolPolicy::default();
    policy.allow_tool_name(TEST_CAPABILITY_NAME);
    ModelTools {
        transport: noema_providers::ProviderToolTransport::Native,
        bindings: builder.build(),
        prompt_rows: Vec::new(),
        unavailable_rows: Vec::new(),
        prompt_kinds: std::collections::BTreeMap::from([(
            TEST_CAPABILITY_NAME.to_string(),
            crate::daemon::runtime::model_tools::ModelToolPromptKind::Capability,
        )]),
        tool_policy: policy,
    }
}

fn test_tool_call(name: &str, payload: Value) -> LocalToolCall {
    LocalToolCall {
        output_index: 0,
        call_id: Some("call:test".to_string()),
        provider_call_id: Some("provider_call:test".to_string()),
        provider_name: Some("provider_tool:test".to_string()),
        name: name.to_string(),
        payload,
    }
}
