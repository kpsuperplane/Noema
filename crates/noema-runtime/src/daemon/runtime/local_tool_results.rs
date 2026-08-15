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
    WebBrowse,
    Gateway,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActionRequestReference {
    pub(super) action_id: String,
    pub(super) revision: u64,
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
    /// Optional richer source for persistence. This value never enters model context.
    pub(super) persisted_output_source: Option<Value>,
    pub(super) success: bool,
    pub(super) side_effect: bool,
    /// Canonical structured result delivered to the model.
    pub(super) payload: Value,
    pub(super) requires_provider_continuation: bool,
    pub(super) blocked_action_request: Option<ActionRequestReference>,
    pub(super) blocked_authentication_id: Option<String>,
    pub(super) blocked_outcome_uncertain: bool,
    pub(super) pending_interaction_id: Option<String>,
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
            persisted_output_source: None,
            success,
            side_effect: false,
            payload,
            requires_provider_continuation,
            blocked_action_request: None,
            blocked_authentication_id: None,
            blocked_outcome_uncertain: false,
            pending_interaction_id: None,
            kind,
        }
    }

    pub(super) fn with_blocked_action_request(mut self, action_id: String, revision: u64) -> Self {
        self.blocked_action_request = Some(ActionRequestReference {
            action_id,
            revision,
        });
        self
    }

    pub(super) fn with_side_effect(mut self, side_effect: bool) -> Self {
        self.side_effect = side_effect;
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

    pub(super) fn with_pending_interaction(mut self, interaction_id: String) -> Self {
        self.pending_interaction_id = Some(interaction_id);
        self
    }

    pub(super) const fn is_blocked(&self) -> bool {
        self.blocked_action_request.is_some()
            || self.blocked_authentication_id.is_some()
            || self.blocked_outcome_uncertain
    }

    pub(super) const fn is_waiting_for_interaction(&self) -> bool {
        self.pending_interaction_id.is_some()
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

    pub(super) fn with_persisted_output_source(mut self, output: Value) -> Self {
        self.persisted_output_source = Some(output);
        self
    }

    pub(super) fn transcript_payload(&self) -> Value {
        json!({
            "call_id": self.call_id,
            "provider_call_id": self.provider_call_id,
            "provider_name": self.provider_name,
            "name": self.name,
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
    if let Some(request) = result.blocked_action_request.as_ref() {
        return GenerateActionItem::ApprovalRequest {
            id: Some(request.action_id.clone()),
            method: result.name.clone(),
            payload: json!({
                "revision": request.revision,
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
