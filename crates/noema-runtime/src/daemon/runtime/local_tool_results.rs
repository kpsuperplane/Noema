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
    /// Canonical structured result delivered to the model.
    pub(super) payload: Value,
    pub(super) requires_provider_continuation: bool,
    pub(super) blocked_action_id: Option<String>,
    pub(super) blocked_authentication_id: Option<String>,
    pub(super) blocked_outcome_uncertain: bool,
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
            blocked_authentication_id: None,
            blocked_outcome_uncertain: false,
            kind,
        }
    }

    pub(super) fn with_blocked_action(mut self, action_id: String) -> Self {
        self.blocked_action_id = Some(action_id);
        self
    }

    pub(super) fn with_blocked_authentication(mut self, request_id: String) -> Self {
        self.blocked_authentication_id = Some(request_id);
        self
    }

    pub(super) fn with_blocked_outcome_uncertain(mut self) -> Self {
        self.blocked_outcome_uncertain = true;
        self
    }

    pub(super) const fn is_blocked(&self) -> bool {
        self.blocked_action_id.is_some()
            || self.blocked_authentication_id.is_some()
            || self.blocked_outcome_uncertain
    }

    pub(super) const fn has_uncertain_outcome(&self) -> bool {
        self.blocked_outcome_uncertain
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
            "arguments": self.persisted.arguments.clone().unwrap_or_else(omitted_payload),
            "success": self.success,
            "payload": self.persisted.output.clone().unwrap_or_else(omitted_payload),
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
    if let Some(request_id) = result.blocked_authentication_id.as_ref() {
        return GenerateActionItem::AuthenticationRequest {
            id: request_id.clone(),
            method: result.name.clone(),
            payload: json!({
                "call_id": result.call_id,
                "provider_call_id": result.provider_call_id,
                "provider_name": result.provider_name,
                "name": result.name,
            }),
        };
    }
    let persisted_payload = result
        .persisted
        .output
        .clone()
        .unwrap_or_else(omitted_payload);
    GenerateActionItem::ToolResult {
        call_id: result.call_id.clone(),
        provider_call_id: result.provider_call_id.clone(),
        provider_name: result.provider_name.clone(),
        name: Some(result.name.clone()),
        success: Some(result.success),
        payload: persisted_payload,
    }
}

fn omitted_payload() -> Value {
    json!({"omitted": true})
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
