use crate::{
    capability::{CapabilityGateway, GatewayToolProposal, GatewayToolResult},
    provider::{DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateActionItem, GenerateToolResultInput},
};
use serde_json::{Value, json};

use super::{
    actor::CodexRuntimeActor, tool_lifecycle::LocalToolCall, turn::SuccessfulProviderTurn,
};
use crate::daemon::{
    agent_name_tool::{
        AgentNameToolResult, AgentNameToolRuntimeContext, execute_update_own_name,
        is_update_own_name_tool,
    },
    agent_onboarding::AgentPromptIdentity,
    memory::tool::{
        MemoryToolResult, MemoryToolRuntimeContext, execute_search_memory, is_search_memory_tool,
    },
};
use crate::search::tool::{WebSearchToolResult, execute_web_search, is_web_search_tool};
use crate::web_fetch::{
    tool::{WebFetchToolResult, execute_web_fetch, is_web_fetch_tool},
    types::FetchRuntimeContext,
};

impl CodexRuntimeActor {
    pub(super) async fn execute_local_tool(
        &self,
        turn: &SuccessfulProviderTurn,
        agent_identity: &AgentPromptIdentity,
        call: &LocalToolCall,
    ) -> LocalToolResult {
        let gateway = CapabilityGateway {
            store: &self.store,
            system_errors: &self.system_errors,
        };
        if is_search_memory_tool(&call.name) {
            let context = MemoryToolRuntimeContext {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                call_site_id: format!("output_{}", call.output_index),
                cwd: turn.cwd.clone(),
                user_input: turn.user_input.clone(),
            };
            LocalToolResult::Memory {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                arguments: call.payload.clone(),
                result: execute_search_memory(
                    &self.store,
                    &context,
                    call.call_id.clone(),
                    &call.payload,
                )
                .await,
            }
        } else if is_update_own_name_tool(&call.name) {
            let context = AgentNameToolRuntimeContext {
                agent_id: agent_identity.agent_id.clone(),
            };
            LocalToolResult::AgentName {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                arguments: call.payload.clone(),
                result: execute_update_own_name(
                    &self.store,
                    &context,
                    call.call_id.clone(),
                    &call.payload,
                )
                .await,
            }
        } else if is_web_search_tool(&call.name) {
            LocalToolResult::WebSearch {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                arguments: call.payload.clone(),
                result: execute_web_search(
                    &self.search_provider,
                    call.call_id.clone(),
                    &call.payload,
                )
                .await,
            }
        } else if is_web_fetch_tool(&call.name) {
            let summarizer_provider = self
                .provider_for_kind(&turn.provider_kind)
                .unwrap_or_else(|_| self.default_provider().expect("runtime default provider"));
            let summarizer_model = turn
                .model
                .clone()
                .or_else(|| summarizer_provider.default_tool_classification_model())
                .unwrap_or_else(|| DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string());
            let context = FetchRuntimeContext {
                summarizer_provider_kind: turn.provider_kind.clone(),
                summarizer_provider,
                summarizer_model,
            };
            LocalToolResult::WebFetch {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                arguments: call.payload.clone(),
                result: execute_web_fetch(
                    &self.web_fetch_provider,
                    &context,
                    call.call_id.clone(),
                    &call.payload,
                )
                .await,
            }
        } else {
            let proposal = GatewayToolProposal {
                name: &call.name,
                payload: &call.payload,
            };
            LocalToolResult::Gateway {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                name: call.name.clone(),
                arguments: call.payload.clone(),
                result: gateway.execute_tool_proposal(proposal).await,
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum LocalToolResult {
    Memory {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        result: MemoryToolResult,
    },
    AgentName {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        result: AgentNameToolResult,
    },
    WebSearch {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        result: WebSearchToolResult,
    },
    WebFetch {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        result: WebFetchToolResult,
    },
    Gateway {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        name: String,
        arguments: Value,
        result: GatewayToolResult,
    },
}

impl LocalToolResult {
    fn call_id(&self) -> Option<&String> {
        match self {
            Self::Memory { call_id, .. }
            | Self::AgentName { call_id, .. }
            | Self::WebSearch { call_id, .. }
            | Self::WebFetch { call_id, .. } => call_id.as_ref(),
            Self::Gateway { call_id, .. } => call_id.as_ref(),
        }
    }

    fn provider_call_id(&self) -> Option<&String> {
        match self {
            Self::Memory {
                provider_call_id, ..
            }
            | Self::AgentName {
                provider_call_id, ..
            }
            | Self::WebSearch {
                provider_call_id, ..
            }
            | Self::WebFetch {
                provider_call_id, ..
            }
            | Self::Gateway {
                provider_call_id, ..
            } => provider_call_id.as_ref(),
        }
    }

    fn provider_name(&self) -> Option<&String> {
        match self {
            Self::Memory { provider_name, .. }
            | Self::AgentName { provider_name, .. }
            | Self::WebSearch { provider_name, .. }
            | Self::WebFetch { provider_name, .. }
            | Self::Gateway { provider_name, .. } => provider_name.as_ref(),
        }
    }

    fn arguments(&self) -> &Value {
        match self {
            Self::Memory { arguments, .. }
            | Self::AgentName { arguments, .. }
            | Self::WebSearch { arguments, .. }
            | Self::WebFetch { arguments, .. }
            | Self::Gateway { arguments, .. } => arguments,
        }
    }

    pub(super) fn name(&self) -> &str {
        match self {
            Self::Memory { result, .. } => &result.name,
            Self::AgentName { result, .. } => &result.name,
            Self::WebSearch { result, .. } => &result.name,
            Self::WebFetch { result, .. } => &result.name,
            Self::Gateway { name, .. } => name,
        }
    }

    pub(super) fn success(&self) -> bool {
        match self {
            Self::Memory { result, .. } => result.success,
            Self::AgentName { result, .. } => result.success,
            Self::WebSearch { result, .. } => result.success,
            Self::WebFetch { result, .. } => result.success,
            Self::Gateway { result, .. } => result.success,
        }
    }

    fn payload(&self) -> &Value {
        match self {
            Self::Memory { result, .. } => &result.payload,
            Self::AgentName { result, .. } => &result.payload,
            Self::WebSearch { result, .. } => &result.payload,
            Self::WebFetch { result, .. } => &result.payload,
            Self::Gateway { result, .. } => &result.payload,
        }
    }

    pub(super) fn requires_provider_continuation(&self) -> bool {
        match self {
            Self::Memory { .. } | Self::WebSearch { .. } | Self::WebFetch { .. } => true,
            Self::AgentName { .. } => false,
            Self::Gateway { result, .. } => result.requires_provider_continuation,
        }
    }

    pub(super) fn native_tool_result_input(&self) -> Option<GenerateToolResultInput> {
        Some(GenerateToolResultInput {
            id: self.call_id().cloned(),
            call_id: self.provider_call_id()?.clone(),
            name: self.name().to_string(),
            provider_name: self.provider_name().cloned(),
            arguments: self.arguments().clone(),
            success: self.success(),
            payload: self.payload().clone(),
        })
    }
}

pub(super) fn agent_identity_after_local_tools(
    current: &AgentPromptIdentity,
    results: &[LocalToolResult],
) -> AgentPromptIdentity {
    let mut agent_identity = current.clone();
    for result in results {
        let LocalToolResult::AgentName { result, .. } = result else {
            continue;
        };
        if result.success {
            agent_identity.display_name = result
                .payload
                .get("display_name")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
    }
    agent_identity
}

pub(super) fn local_tool_result_continuation_input(results: &[&LocalToolResult]) -> Value {
    json!({
        "type": "NOEMA_LOCAL_TOOL_RESULT",
        "results": results
            .iter()
            .map(|result| local_tool_result_payload(result))
            .collect::<Vec<_>>(),
    })
}

fn local_tool_result_payload(result: &LocalToolResult) -> Value {
    json!({
        "call_id": result.call_id(),
        "provider_call_id": result.provider_call_id(),
        "provider_name": result.provider_name(),
        "name": result.name(),
        "success": result.success(),
        "payload": result.payload(),
    })
}

pub(super) fn local_tool_result_action_item(result: &LocalToolResult) -> GenerateActionItem {
    GenerateActionItem::ToolResult {
        call_id: result.call_id().cloned(),
        provider_call_id: result.provider_call_id().cloned(),
        provider_name: result.provider_name().cloned(),
        name: Some(result.name().to_string()),
        success: Some(result.success()),
        payload: result.payload().clone(),
    }
}
