use noema_conversations::{
    ActorRef, AgentStatus as PersistedAgentStatus, ConversationItemKind, ConversationItemRecord,
    ConversationItemStatus, NewConversationItem,
};

use noema_providers::{
    AssistantTextPhase, GenerateActionItem, GenerateCitation, GenerateHostedWebSearch,
    GenerateReasoningItem, GenerateResponse, GenerateResponseItem, GenerateStreamEvent,
};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::{
    actor::RuntimeActor,
    citation_markers::CitationSourceRegistry,
    tool_lifecycle::{LocalToolCall, tool_call_action_item},
    turn::{
        ProviderActionOutput, ProviderActionTurn, ProviderAssistantResponse,
        ProviderResponsePosition,
    },
};
use crate::daemon::{
    memory::context::ConversationMemoryContext,
    protocol::{RuntimeError, TurnActivityStatus, TurnStreamEvent, TurnTranscriptItem},
};

pub(in crate::daemon::runtime) fn provider_usage_metadata(
    provider: &str,
    model: &str,
    phase: &'static str,
    position: ProviderResponsePosition,
    usage: Option<&noema_providers::TokenUsage>,
) -> Value {
    let Some(usage) = usage else {
        return json!({});
    };

    let mut provider_usage = serde_json::Map::new();
    provider_usage.insert("provider".to_string(), json!(provider));
    provider_usage.insert("model".to_string(), json!(model));
    provider_usage.insert("phase".to_string(), json!(phase));
    provider_usage.insert("response_index".to_string(), json!(position.response_index));
    if let Some(output_index) = position.output_index {
        provider_usage.insert("output_index".to_string(), json!(output_index));
    }
    provider_usage.insert("input_tokens".to_string(), json!(usage.input_tokens));
    provider_usage.insert("output_tokens".to_string(), json!(usage.output_tokens));
    provider_usage.insert("total_tokens".to_string(), json!(usage.total_tokens));
    if let Some(cached_input_tokens) = usage.cached_input_tokens {
        provider_usage.insert(
            "cached_input_tokens".to_string(),
            json!(cached_input_tokens),
        );
        if usage.input_tokens > 0 {
            provider_usage.insert(
                "cache_hit_ratio".to_string(),
                json!(cached_input_tokens as f64 / usage.input_tokens as f64),
            );
        }
    }

    json!({ "provider_usage": Value::Object(provider_usage) })
}

fn merge_metadata(base: &mut Value, extra: Value) {
    let Some(base) = base.as_object_mut() else {
        return;
    };
    let Value::Object(extra) = extra else {
        return;
    };
    for (key, value) in extra {
        base.insert(key, value);
    }
}

include!("transcript_persistence/provider_items.rs");
include!("transcript_persistence/tool_lifecycle.rs");
include!("transcript_persistence/action_items.rs");
include!("transcript_persistence/turn_items.rs");
include!("transcript_persistence/stream_events.rs");
include!("transcript_persistence/tool_call_display.rs");
include!("transcript_persistence/tool_result_display.rs");
include!("transcript_persistence/display_labels.rs");

#[cfg(test)]
#[path = "transcript_persistence/tests.rs"]
mod tests;
