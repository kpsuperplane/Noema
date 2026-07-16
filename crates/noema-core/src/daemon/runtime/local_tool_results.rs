//! Local capability result representation and transcript projections.

use crate::{
    daemon::{
        agent_name_tool::AgentNameToolResult, agent_onboarding::AgentPromptIdentity,
        artifact_tool::ArtifactToolResult, memory::tool::MemoryToolResult,
        protocol::TurnTranscriptItem,
    },
    search::tool::WebSearchToolResult,
    web_fetch::tool::WebFetchToolResult,
};
use noema_providers::GenerateActionItem;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub(super) struct RuntimeCapabilityResult {
    pub(super) success: bool,
    pub(super) payload: Value,
    pub(super) requires_provider_continuation: bool,
}

#[derive(Debug, Clone)]
pub(super) enum LocalToolResult {
    Memory {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        persisted: noema_capabilities::PersistedCapabilityPayload,
        result: MemoryToolResult,
    },
    AgentName {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        persisted: noema_capabilities::PersistedCapabilityPayload,
        result: AgentNameToolResult,
    },
    Artifact {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        persisted: noema_capabilities::PersistedCapabilityPayload,
        result: ArtifactToolResult,
    },
    WebSearch {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        persisted: noema_capabilities::PersistedCapabilityPayload,
        result: WebSearchToolResult,
    },
    WebFetch {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        persisted: noema_capabilities::PersistedCapabilityPayload,
        result: WebFetchToolResult,
    },
    Gateway {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        name: String,
        arguments: Value,
        persisted: noema_capabilities::PersistedCapabilityPayload,
        result: RuntimeCapabilityResult,
    },
}

impl LocalToolResult {
    pub(super) fn persisted(&self) -> &noema_capabilities::PersistedCapabilityPayload {
        match self {
            Self::Memory { persisted, .. }
            | Self::AgentName { persisted, .. }
            | Self::Artifact { persisted, .. }
            | Self::WebSearch { persisted, .. }
            | Self::WebFetch { persisted, .. }
            | Self::Gateway { persisted, .. } => persisted,
        }
    }

    pub(super) fn set_persisted(&mut self, views: noema_capabilities::PersistedCapabilityPayload) {
        match self {
            Self::Memory { persisted, .. }
            | Self::AgentName { persisted, .. }
            | Self::Artifact { persisted, .. }
            | Self::WebSearch { persisted, .. }
            | Self::WebFetch { persisted, .. }
            | Self::Gateway { persisted, .. } => *persisted = views,
        }
    }

    pub(super) fn call_id(&self) -> Option<&String> {
        match self {
            Self::Memory { call_id, .. }
            | Self::AgentName { call_id, .. }
            | Self::Artifact { call_id, .. }
            | Self::WebSearch { call_id, .. }
            | Self::WebFetch { call_id, .. }
            | Self::Gateway { call_id, .. } => call_id.as_ref(),
        }
    }

    pub(super) fn provider_call_id(&self) -> Option<&String> {
        match self {
            Self::Memory {
                provider_call_id, ..
            }
            | Self::AgentName {
                provider_call_id, ..
            }
            | Self::Artifact {
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

    pub(super) fn provider_name(&self) -> Option<&String> {
        match self {
            Self::Memory { provider_name, .. }
            | Self::AgentName { provider_name, .. }
            | Self::Artifact { provider_name, .. }
            | Self::WebSearch { provider_name, .. }
            | Self::WebFetch { provider_name, .. }
            | Self::Gateway { provider_name, .. } => provider_name.as_ref(),
        }
    }

    pub(super) fn arguments(&self) -> &Value {
        match self {
            Self::Memory { arguments, .. }
            | Self::AgentName { arguments, .. }
            | Self::Artifact { arguments, .. }
            | Self::WebSearch { arguments, .. }
            | Self::WebFetch { arguments, .. }
            | Self::Gateway { arguments, .. } => arguments,
        }
    }

    pub(super) fn name(&self) -> &str {
        match self {
            Self::Memory { result, .. } => &result.name,
            Self::AgentName { result, .. } => &result.name,
            Self::Artifact { result, .. } => &result.name,
            Self::WebSearch { result, .. } => &result.name,
            Self::WebFetch { result, .. } => &result.name,
            Self::Gateway { name, .. } => name,
        }
    }

    pub(super) fn success(&self) -> bool {
        match self {
            Self::Memory { result, .. } => result.success,
            Self::AgentName { result, .. } => result.success,
            Self::Artifact { result, .. } => result.success,
            Self::WebSearch { result, .. } => result.success,
            Self::WebFetch { result, .. } => result.success,
            Self::Gateway { result, .. } => result.success,
        }
    }

    pub(super) fn payload(&self) -> &Value {
        match self {
            Self::Memory { result, .. } => &result.payload,
            Self::AgentName { result, .. } => &result.payload,
            Self::Artifact { result, .. } => &result.payload,
            Self::WebSearch { result, .. } => &result.payload,
            Self::WebFetch { result, .. } => &result.payload,
            Self::Gateway { result, .. } => &result.payload,
        }
    }

    pub(super) fn transcript_payload(&self) -> Value {
        json!({
            "call_id": self.call_id(),
            "provider_call_id": self.provider_call_id(),
            "provider_name": self.provider_name(),
            "name": self.name(),
            "arguments": self.arguments(),
            "success": self.success(),
            "payload": self.payload(),
        })
    }

    pub(super) fn requires_provider_continuation(&self) -> bool {
        match self {
            Self::Memory { .. }
            | Self::Artifact { .. }
            | Self::WebSearch { .. }
            | Self::WebFetch { .. }
            | Self::AgentName { .. } => true,
            Self::Gateway { result, .. } => result.requires_provider_continuation,
        }
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
        "results": results.iter().map(|result| local_tool_result_payload(result)).collect::<Vec<_>>(),
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

pub(super) fn local_tool_artifact_reference_item(
    result: &LocalToolResult,
) -> Option<TurnTranscriptItem> {
    if !matches!(result, LocalToolResult::Artifact { .. }) || !result.success() {
        return None;
    }
    let payload = result.payload();
    Some(TurnTranscriptItem::ArtifactReference {
        artifact_id: payload.get("artifact_id")?.as_str()?.to_string(),
        artifact_version_id: payload
            .get("current_version_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        title: payload.get("title")?.as_str()?.to_string(),
        artifact_kind: payload.get("artifact_kind")?.as_str()?.to_string(),
        storage_kind: payload.get("storage_kind")?.as_str()?.to_string(),
        external_url: None,
        download_url: payload
            .get("download_url")
            .and_then(Value::as_str)
            .map(str::to_string),
        media_type: payload
            .get("media_type")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

pub(super) fn local_tool_task_reference_item(
    result: &LocalToolResult,
) -> Option<TurnTranscriptItem> {
    if !matches!(
        result.name(),
        crate::daemon::task_tool::TASK_DELEGATE_TOOL
            | crate::daemon::task_tool::TASK_RESUME_TOOL
            | crate::daemon::task_tool::TASK_CANCEL_TOOL
    ) || !result.success()
    {
        return None;
    }
    let payload = result.payload();
    let status = payload
        .get("status")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<noema_tasks::TaskStatus>().ok())?;
    Some(TurnTranscriptItem::TaskReference {
        task_id: payload.get("task_id")?.as_str()?.to_string(),
        title: payload.get("title")?.as_str()?.to_string(),
        status: status.as_str().to_string(),
        revision: payload
            .get("revision")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
    })
}
