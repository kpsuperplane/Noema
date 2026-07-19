//! Runtime composition for durable governed actions.

use noema_capabilities::{
    CapabilityBinding, CapabilityEffect, CapabilityError, GovernedCapabilityAdmission,
};
use noema_store::{
    GovernedActionEffect, GovernedActionRecord, GovernedActionState, NewGovernedAction,
};

use super::{
    actor::RuntimeActor,
    local_tool_results::{LocalToolKind, LocalToolResult},
    tool_lifecycle::LocalToolCall,
    turn::SuccessfulProviderTurn,
};
use crate::daemon::agent_onboarding::AgentPromptIdentity;

pub(super) enum GovernedActionPreparation {
    NotRequired,
    AwaitingApproval(GovernedActionRecord),
    Admitted {
        action: GovernedActionRecord,
        admission: GovernedCapabilityAdmission,
    },
}

impl RuntimeActor {
    pub(super) async fn prepare_governed_action(
        &self,
        turn: &SuccessfulProviderTurn,
        agent_identity: &AgentPromptIdentity,
        call: &LocalToolCall,
        binding: &CapabilityBinding,
    ) -> Result<GovernedActionPreparation, noema_store::StoreError> {
        let Some(effect) = governed_effect(binding.access().effect) else {
            return Ok(GovernedActionPreparation::NotRequired);
        };
        let action = self
            .store
            .create_governed_action(NewGovernedAction {
                owner_human_id: "human:local".to_string(),
                conversation_id: turn
                    .task_run_id
                    .is_none()
                    .then(|| turn.conversation_id.clone()),
                turn_id: turn.task_run_id.is_none().then(|| turn.turn_id.clone()),
                task_id: turn.task_id.clone(),
                run_id: turn.task_run_id.clone(),
                requesting_agent_id: agent_identity.agent_id.clone(),
                capability_name: call.name.clone(),
                operation_token: binding.target().operation_token().as_str().to_string(),
                effect,
                arguments: call.payload.clone(),
                input_schema: binding.spec().input_schema.as_value().clone(),
                trusted_authority: serde_json::json!({
                    "origin": if turn.task_run_id.is_some() { "task" } else { "primary_conversation" },
                    "human_or_task_request": turn.user_input,
                    "conversation_id": turn.task_run_id.is_none().then_some(&turn.conversation_id),
                    "task_id": turn.task_id,
                    "run_id": turn.task_run_id,
                }),
                safe_summary: safe_action_summary(&call.name, effect),
            })
            .await?;
        let assessment = self.review_governed_action(&action, turn).await;
        let action = self
            .store
            .record_governed_action_assessment(&action.action_id, action.revision, assessment)
            .await?;
        if action.state == GovernedActionState::AwaitingApproval {
            return Ok(GovernedActionPreparation::AwaitingApproval(action));
        }
        let action = self
            .store
            .claim_governed_action_execution(&action.action_id, action.revision)
            .await?;
        let admission = GovernedCapabilityAdmission {
            action_id: action.action_id.clone(),
            revision: action.revision,
            arguments_sha256: action.arguments_sha256.clone(),
        };
        Ok(GovernedActionPreparation::Admitted { action, admission })
    }
}

fn governed_effect(effect: CapabilityEffect) -> Option<GovernedActionEffect> {
    match effect {
        CapabilityEffect::ExternalWrite => Some(GovernedActionEffect::Write),
        CapabilityEffect::ExternalExport => Some(GovernedActionEffect::Export),
        CapabilityEffect::ExternalWriteAndExport => Some(GovernedActionEffect::WriteAndExport),
        CapabilityEffect::ReadOnly | CapabilityEffect::Mutating | CapabilityEffect::Internal => {
            None
        }
    }
}

fn safe_action_summary(capability_name: &str, effect: GovernedActionEffect) -> String {
    let effect = match effect {
        GovernedActionEffect::Write => "write external data",
        GovernedActionEffect::Export => "send data to an external destination",
        GovernedActionEffect::WriteAndExport => "write and send external data",
    };
    format!("{capability_name} wants to {effect}")
}

pub(super) fn awaiting_approval_result(
    call: &LocalToolCall,
    action: &GovernedActionRecord,
) -> LocalToolResult {
    LocalToolResult::from_call(
        call,
        LocalToolKind::Gateway,
        false,
        serde_json::json!({
            "status": "awaiting_approval",
            "action_id": action.action_id,
            "revision": action.revision,
            "summary": action.safe_summary,
        }),
        false,
    )
    .with_blocked_action(action.action_id.clone())
}

pub(super) fn action_store_failure_result(call: &LocalToolCall) -> LocalToolResult {
    LocalToolResult::from_call(
        call,
        LocalToolKind::Gateway,
        false,
        serde_json::json!({"error": "governed action could not be persisted"}),
        false,
    )
}

pub(super) fn capability_failure_code(error: &CapabilityError) -> &'static str {
    match error {
        CapabilityError::UnknownInvoker => "unknown_invoker",
        CapabilityError::UnknownOperation => "unknown_operation",
        CapabilityError::InvalidArguments => "invalid_arguments",
        CapabilityError::Denied => "denied",
        CapabilityError::Unavailable => "unavailable",
        CapabilityError::Failed => "failed",
        CapabilityError::OutcomeUncertain => "outcome_uncertain",
    }
}
