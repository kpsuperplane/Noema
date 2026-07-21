//! Local capability result representation and transcript projections.

use crate::daemon::{agent_onboarding::AgentPromptIdentity, protocol::TurnTranscriptItem};
use noema_providers::GenerateActionItem;
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LocalToolKind {
    Memory,
    AgentName,
    Artifact,
    WebSearch,
    WebFetch,
    Gateway,
}

/// One normalized result envelope for every runtime-owned capability.
#[derive(Debug, Clone)]
pub(super) struct LocalToolResult {
    pub(super) call_id: Option<String>,
    pub(super) provider_call_id: Option<String>,
    pub(super) provider_name: Option<String>,
    pub(super) name: String,
    pub(super) arguments: Value,
    pub(super) persisted: noema_capabilities::PersistedCapabilityPayload,
    pub(super) success: bool,
    pub(super) payload: Value,
    pub(super) requires_provider_continuation: bool,
    pub(super) blocked_action_id: Option<String>,
    pub(super) kind: LocalToolKind,
}

impl LocalToolResult {
    pub(super) fn from_call(
        call: &super::tool_lifecycle::LocalToolCall,
        kind: LocalToolKind,
        success: bool,
        payload: Value,
        requires_provider_continuation: bool,
    ) -> Self {
        Self {
            call_id: call.call_id.clone(),
            provider_call_id: call.provider_call_id.clone(),
            provider_name: call.provider_name.clone(),
            name: call.name.clone(),
            arguments: call.payload.clone(),
            persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
            success,
            payload,
            requires_provider_continuation,
            blocked_action_id: None,
            kind,
        }
    }

    pub(super) fn with_blocked_action(mut self, action_id: String) -> Self {
        self.blocked_action_id = Some(action_id);
        self
    }

    pub(super) fn with_persisted(
        mut self,
        persisted: noema_capabilities::PersistedCapabilityPayload,
    ) -> Self {
        self.persisted = persisted;
        self
    }

    pub(super) fn transcript_payload(&self) -> Value {
        json!({
            "call_id": self.call_id,
            "provider_call_id": self.provider_call_id,
            "provider_name": self.provider_name,
            "name": self.name,
            "arguments": self.arguments,
            "success": self.success,
            "payload": self.payload,
        })
    }
}

pub(super) fn agent_identity_after_local_tools(
    current: &AgentPromptIdentity,
    results: &[LocalToolResult],
) -> AgentPromptIdentity {
    let mut agent_identity = current.clone();
    for result in results {
        if result.kind == LocalToolKind::AgentName && result.success {
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
        "call_id": result.call_id,
        "provider_call_id": result.provider_call_id,
        "provider_name": result.provider_name,
        "name": result.name,
        "success": result.success,
        "payload": result.payload,
    })
}

pub(super) fn local_tool_result_action_item(result: &LocalToolResult) -> GenerateActionItem {
    if let Some(action_id) = result.blocked_action_id.as_ref() {
        return GenerateActionItem::ApprovalRequest {
            id: Some(action_id.clone()),
            method: result.name.clone(),
            payload: json!({
                "call_id": result.call_id,
                "provider_call_id": result.provider_call_id,
                "provider_name": result.provider_name,
                "name": result.name,
            }),
        };
    }
    let persisted_payload = if result.kind == LocalToolKind::Memory {
        if result.name == noema_memory::READ_MEMORY_PAGE_TOOL_NAME {
            result
                .payload
                .get("page")
                .map(|page| {
                    json!({
                        "page_ref": {
                            "id": page.get("id"),
                            "path": page.get("path"),
                            "hash": page.get("hash"),
                        }
                    })
                })
                .unwrap_or_else(|| json!({"page_ref": null}))
        } else if result.name == noema_memory::NATIVE_SEARCH_MEMORY_TOOL_NAME {
            let pages = result
                .payload
                .get("pages")
                .and_then(Value::as_array)
                .map(|pages| {
                    pages
                        .iter()
                        .map(|page| {
                            json!({
                                "id": page.get("id"),
                                "path": page.get("path"),
                                "hash": page.get("hash"),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            json!({"pages": pages})
        } else {
            json!({"memory_result": "omitted"})
        }
    } else {
        result.payload.clone()
    };
    GenerateActionItem::ToolResult {
        call_id: result.call_id.clone(),
        provider_call_id: result.provider_call_id.clone(),
        provider_name: result.provider_name.clone(),
        name: Some(result.name.clone()),
        success: Some(result.success),
        payload: persisted_payload,
    }
}

pub(super) fn local_tool_artifact_reference_item(
    result: &LocalToolResult,
) -> Option<TurnTranscriptItem> {
    if result.kind != LocalToolKind::Artifact || !result.success {
        return None;
    }
    let payload = &result.payload;
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
    if !crate::daemon::task_tool::is_primary_task_tool(&result.name) || !result.success {
        return None;
    }
    let task = result.payload.get("task")?;
    Some(TurnTranscriptItem::TaskReference {
        task_id: task.get("task_id")?.as_str()?.to_string(),
    })
}
