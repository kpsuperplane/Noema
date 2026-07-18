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
            kind,
        }
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
    GenerateActionItem::ToolResult {
        call_id: result.call_id.clone(),
        provider_call_id: result.provider_call_id.clone(),
        provider_name: result.provider_name.clone(),
        name: Some(result.name.clone()),
        success: Some(result.success),
        payload: result.payload.clone(),
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
    if !matches!(
        result.name.as_str(),
        crate::daemon::task_tool::TASK_DELEGATE_TOOL
            | crate::daemon::task_tool::TASK_RESUME_TOOL
            | crate::daemon::task_tool::TASK_CANCEL_TOOL
    ) || !result.success
    {
        return None;
    }
    let payload = &result.payload;
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
