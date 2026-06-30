use crate::provider::GenerateOutputItem;
use serde_json::{Value, json};

use super::{actor::CodexRuntimeActor, turn::SuccessfulProviderTurn};
use crate::daemon::{
    agent_name_tool::{
        AgentNameToolResult, AgentNameToolRuntimeContext, execute_update_own_name,
        is_update_own_name_tool,
    },
    agent_onboarding::AgentPromptIdentity,
    memory_tool::{
        MemoryToolResult, MemoryToolRuntimeContext, execute_search_memory, is_search_memory_tool,
    },
};

impl CodexRuntimeActor {
    pub(super) async fn execute_local_tools(
        &self,
        turn: &SuccessfulProviderTurn,
        agent_identity: &AgentPromptIdentity,
    ) -> Vec<LocalToolResult> {
        let mut results = Vec::new();
        for (index, output) in turn.response.output.iter().enumerate() {
            let GenerateOutputItem::ToolCall { id, name, payload } = output else {
                continue;
            };
            if is_search_memory_tool(name) {
                let context = MemoryToolRuntimeContext {
                    conversation_id: turn.conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    turn_index: turn.turn_index,
                    call_site_id: format!("output_{index}"),
                    cwd: turn.cwd.clone(),
                    user_input: turn.user_input.clone(),
                };
                results.push(LocalToolResult::Memory(
                    execute_search_memory(&self.store, &context, id.clone(), payload).await,
                ));
            } else if is_update_own_name_tool(name) {
                let context = AgentNameToolRuntimeContext {
                    agent_id: agent_identity.agent_id.clone(),
                };
                results.push(LocalToolResult::AgentName(
                    execute_update_own_name(&self.store, &context, id.clone(), payload).await,
                ));
            }
        }
        results
    }
}

#[derive(Debug, Clone)]
pub(super) enum LocalToolResult {
    Memory(MemoryToolResult),
    AgentName(AgentNameToolResult),
}

impl LocalToolResult {
    fn call_id(&self) -> Option<&String> {
        match self {
            Self::Memory(result) => result.call_id.as_ref(),
            Self::AgentName(result) => result.call_id.as_ref(),
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Memory(result) => &result.name,
            Self::AgentName(result) => &result.name,
        }
    }

    fn success(&self) -> bool {
        match self {
            Self::Memory(result) => result.success,
            Self::AgentName(result) => result.success,
        }
    }

    fn payload(&self) -> &Value {
        match self {
            Self::Memory(result) => &result.payload,
            Self::AgentName(result) => &result.payload,
        }
    }

    pub(super) fn requires_provider_continuation(&self) -> bool {
        matches!(self, Self::Memory(_))
    }
}

pub(super) fn agent_identity_after_local_tools(
    current: &AgentPromptIdentity,
    results: &[LocalToolResult],
) -> AgentPromptIdentity {
    let mut agent_identity = current.clone();
    for result in results {
        let LocalToolResult::AgentName(result) = result else {
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
        "name": result.name(),
        "success": result.success(),
        "payload": result.payload(),
    })
}

pub(super) fn local_tool_result_output_item(result: &LocalToolResult) -> GenerateOutputItem {
    GenerateOutputItem::ToolResult {
        call_id: result.call_id().cloned(),
        name: Some(result.name().to_string()),
        success: Some(result.success()),
        payload: result.payload().clone(),
    }
}
