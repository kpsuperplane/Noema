use super::*;
use super::{protocol::TurnStreamEvent, runtime::RuntimeHandle};
use noema_conversations::{ActorRef, ConversationItemKind, ConversationItemStatus, ReplayMode};
use noema_providers::{
    AssistantTextPhase, GenerateActionItem, GenerateInput, GenerateInputItem,
    GenerateReasoningItem, GenerateRequest, GenerateResponse, GenerateResponseItem,
    GenerateStreamEvent, GenerateToolCall, ProviderError, ProviderResponseContinuation,
    ProviderToolCapabilities, ProviderToolSchemaDialect, ProviderToolTransport,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    future::Future,
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};

fn read_system_error_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .expect("system error log")
        .lines()
        .map(|line| serde_json::from_str(line).expect("system error event"))
        .collect()
}

async fn upsert_ready_agent_runtime_preference(
    store: &noema_store::NoemaStore,
    preference: noema_store::NewAgentRuntimePreference,
) {
    let ready_selection = crate::test_support::ready_provider_selection(
        noema_providers::ProviderSelectionSnapshot::explicit(
            &preference.provider_kind,
            &preference.provider_account_id,
            preference
                .selection
                .model_profile()
                .expect("explicit profile"),
            preference.selection.reasoning_effort(),
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
            preference
                .selection
                .model_profile()
                .expect("explicit profile"),
            preference.selection.reasoning_effort(),
            Some(format!("auxiliary_model_preference:{}", preference.task)),
        ),
    );
    store
        .upsert_auxiliary_model_preference_with_ready_selection(preference, &ready_selection)
        .await
        .expect("ready auxiliary model preference");
}

fn is_compaction_request(request: &GenerateRequest) -> bool {
    request.instructions.as_deref().is_some_and(|instructions| {
        instructions.contains("Compact Noema conversation context")
            || instructions.contains("Compact an active Noema agent execution")
    })
}

include!("tests/runtime_lifecycle.rs");
include!("tests/conversation_turns.rs");
include!("tests/prompt_context.rs");
include!("tests/compaction_routing.rs");
include!("tests/replay_and_tools.rs");
include!("tests/continuation_and_web.rs");
include!("tests/identity_and_memory.rs");
include!("tests/support/runtime.rs");
include!("tests/support/providers.rs");
include!("tests/support/input.rs");
include!("tests/support/provider_operations.rs");
include!("tests/support/output.rs");
