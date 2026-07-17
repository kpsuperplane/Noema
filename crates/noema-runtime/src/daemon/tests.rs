use super::*;
use super::{protocol::TurnStreamEvent, runtime::RuntimeHandle};
use noema_conversations::{ActorRef, ConversationItemKind, ConversationItemStatus, ReplayMode};
use noema_home::NoemaPaths;
use noema_providers::{
    AssistantTextPhase, GenerateActionItem, GenerateInput, GenerateInputItem,
    GenerateReasoningItem, GenerateRequest, GenerateResponse, GenerateResponseItem,
    GenerateResponseStatus, GenerateStreamEvent, GenerateToolCall, MultipleChoiceOption,
    MultipleChoiceSelectionMode, ProviderError, ProviderResponseContinuation,
    ProviderToolCapabilities, ProviderToolSchemaDialect, ProviderToolTransport, WebFetchBackend,
    WebFetchBackendHandle, WebFetchContext, WebFetchError, WebOperationFuture, WebSearchBackend,
    WebSearchBackendHandle, WebSearchError,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{Mutex as AsyncMutex, Notify, mpsc, oneshot},
};

fn read_system_error_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .expect("system error log")
        .lines()
        .map(|line| serde_json::from_str(line).expect("system error event"))
        .collect()
}

const RESTART_CONTEXT_TEST_PHASE_ENV: &str = "NOEMA_RESTART_CONTEXT_TEST_PHASE";
const RESTART_CONTEXT_TEST_HOME_ENV: &str = "NOEMA_RESTART_CONTEXT_TEST_HOME";
const RESTART_CONTEXT_TEST_CONVERSATION_FILE: &str = "restart_context_conversation_id";

async fn upsert_ready_agent_runtime_preference(
    store: &noema_store::NoemaStore,
    preference: noema_store::NewAgentRuntimePreference,
) {
    let ready_selection = crate::test_support::ready_provider_selection(
        noema_providers::ProviderSelectionSnapshot::explicit(
            &preference.provider_kind,
            &preference.provider_account_id,
            &preference.model_profile,
            preference.reasoning_effort,
            Some(format!("agent_runtime_preference:{}", preference.agent_id)),
        ),
    );
    store
        .upsert_agent_runtime_preference_with_ready_selection(preference, &ready_selection)
        .await
        .expect("ready agent runtime preference");
}

async fn upsert_ready_auxiliary_model_preference(
    store: &noema_store::NoemaStore,
    preference: noema_store::NewAuxiliaryModelPreference,
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

#[derive(Debug, Clone)]
struct StaticWebSearchBackend {
    response: noema_capabilities::web::search::SearchResponse,
}

impl WebSearchBackend for StaticWebSearchBackend {
    fn backend_id(&self) -> &str {
        &self.response.provider
    }

    fn search<'a>(
        &'a self,
        request: &'a noema_capabilities::web::search::SearchRequest,
    ) -> WebOperationFuture<'a, noema_capabilities::web::search::SearchResponse, WebSearchError>
    {
        Box::pin(async move {
            let mut response = self.response.clone();
            response.query = request.query.clone();
            response.results.truncate(request.max_results);
            response.summary = match response.results.len() {
                0 => "No web results found".to_string(),
                1 => "Found 1 web result".to_string(),
                count => format!("Found {count} web results"),
            };
            Ok(response)
        })
    }
}

fn static_web_search_backend(
    response: noema_capabilities::web::search::SearchResponse,
) -> WebSearchBackendHandle {
    WebSearchBackendHandle::new(StaticWebSearchBackend { response })
}

#[derive(Debug, Clone)]
struct StaticWebFetchBackend {
    response: noema_capabilities::web::fetch::FetchResponse,
}

impl WebFetchBackend for StaticWebFetchBackend {
    fn backend_id(&self) -> &str {
        &self.response.provider
    }

    fn fetch<'a>(
        &'a self,
        request: &'a noema_capabilities::web::fetch::FetchRequest,
        _context: &'a WebFetchContext,
    ) -> WebOperationFuture<'a, noema_capabilities::web::fetch::FetchResponse, WebFetchError> {
        Box::pin(async move {
            let mut response = self.response.clone();
            response.url = request.url.clone();
            response.returned_chars = response.content.chars().count();
            Ok(response)
        })
    }
}

fn static_web_fetch_backend(
    response: noema_capabilities::web::fetch::FetchResponse,
) -> WebFetchBackendHandle {
    WebFetchBackendHandle::new(StaticWebFetchBackend { response })
}

#[derive(Debug, Clone)]
enum GenerateOutputItem {
    AssistantText {
        phase: Option<AssistantTextPhase>,
        text: String,
    },
    MultipleChoice {
        phase: Option<AssistantTextPhase>,
        prompt: String,
        selection_mode: MultipleChoiceSelectionMode,
        options: Vec<MultipleChoiceOption>,
    },
    ToolCall {
        id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        name: String,
        payload: serde_json::Value,
    },
}

include!("tests/runtime_lifecycle.rs");
include!("tests/conversation_turns.rs");
include!("tests/prompt_context.rs");
include!("tests/compaction_routing.rs");
include!("tests/replay_and_tools.rs");
include!("tests/memory.rs");
include!("tests/continuation_and_web.rs");
include!("tests/identity_and_memory.rs");
include!("tests/support/runtime.rs");
include!("tests/support/providers.rs");
include!("tests/support/input.rs");
include!("tests/support/provider_operations.rs");
include!("tests/support/output.rs");
