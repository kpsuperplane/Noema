//! Runtime composition for durable governed actions.

use noema_capabilities::{
    CapabilityAdmissionPolicy, CapabilityBinding, CapabilityEffect, CapabilityError,
    GovernedCapabilityAdmission,
};
use noema_store::{
    GovernedActionEffect, GovernedActionRecord, GovernedActionState, GovernedAssessmentStatus,
    NewGovernedAction, NewGovernedActionAssessment,
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
        action: Option<GovernedActionRecord>,
        admission: GovernedCapabilityAdmission,
        arguments: Option<serde_json::Value>,
    },
}

impl RuntimeActor {
    pub(super) async fn prepare_governed_action(
        &self,
        turn: &SuccessfulProviderTurn,
        agent_identity: &AgentPromptIdentity,
        call: &LocalToolCall,
        binding: &CapabilityBinding,
        result_route_fence: serde_json::Value,
    ) -> Result<GovernedActionPreparation, noema_store::StoreError> {
        if !binding.admission_policy().requires_governed_admission() {
            return Ok(GovernedActionPreparation::NotRequired);
        }
        let Some(effect) = governed_effect(binding.access().effect) else {
            return Ok(GovernedActionPreparation::NotRequired);
        };
        if call.name == noema_capabilities::web::fetch::WEB_FETCH_TOOL
            && let Some(arguments) = observed_fetch_arguments(&self.store, &call.payload).await?
        {
            return Ok(GovernedActionPreparation::Admitted {
                action: None,
                admission: GovernedCapabilityAdmission::for_observed_url(&arguments),
                arguments: Some(arguments),
            });
        }
        let authorization_context =
            action_authorization_context(&self.store, turn, binding, result_route_fence).await?;
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
                authorization_context,
                safe_summary: safe_action_summary(&call.name, effect),
            })
            .await?;
        let assessment = match binding.admission_policy() {
            CapabilityAdmissionPolicy::Direct
            | CapabilityAdmissionPolicy::PolicyAuthorizedDirect => {
                unreachable!("direct admission returned above")
            }
            CapabilityAdmissionPolicy::ReviewerMayApprove => {
                self.review_governed_action(&action, turn).await
            }
            CapabilityAdmissionPolicy::AlwaysAsk => NewGovernedActionAssessment {
                status: GovernedAssessmentStatus::ReviewerUnavailable,
                reviewer_selection: None,
                authorization: None,
                risk: None,
                reason_codes: vec!["provider_policy_requires_approval".to_string()],
                explanation: "Provider policy requires human approval for this call.".to_string(),
            },
        };
        let action = self
            .store
            .record_governed_action_assessment(
                &action.action_id,
                action.revision,
                assessment,
                turn.task_run_fence.as_ref(),
            )
            .await?;
        if action.state == GovernedActionState::AwaitingApproval {
            return Ok(GovernedActionPreparation::AwaitingApproval(action));
        }
        let action = self
            .store
            .claim_governed_action_execution(
                &action.action_id,
                action.revision,
                turn.task_run_fence.as_ref(),
            )
            .await?;
        let admission = GovernedCapabilityAdmission::for_action(
            action.action_id.clone(),
            action.revision,
            &action.arguments,
        );
        Ok(GovernedActionPreparation::Admitted {
            action: Some(action),
            admission,
            arguments: None,
        })
    }
}

async fn action_authorization_context(
    store: &noema_store::NoemaStore,
    turn: &SuccessfulProviderTurn,
    binding: &CapabilityBinding,
    result_route_fence: serde_json::Value,
) -> Result<serde_json::Value, noema_store::StoreError> {
    let destination = binding
        .destination()
        .map(|destination| serde_json::to_value(destination).expect("destination is serializable"));
    let result_policy = serde_json::to_value(binding.result_policy())
        .expect("capability result policy is serializable");
    let Some(run_id) = turn.task_run_id.as_deref() else {
        let context = store
            .conversation_authorization_context(
                &turn.conversation_id,
                &turn.turn_id,
                &turn.user_item_id,
            )
            .await?;
        return Ok(serde_json::json!({
            "origin": "primary_conversation",
            "context": context,
            "conversation_id": turn.conversation_id,
            "source_human_item_id": turn.user_item_id,
            "destination": destination,
            "admission_policy": admission_policy_name(binding.admission_policy()),
            "result_route": result_route_fence,
            "result_policy": result_policy,
        }));
    };
    let context = store
        .get_work_run_execution_context(run_id)
        .await?
        .ok_or_else(|| noema_store::StoreError::InvariantViolation {
            message: "governed action has no exact task context".to_string(),
        })?;
    Ok(serde_json::json!({
        "origin": "task",
        "context": context.task.authorization_context,
        "task_id": context.task.task_id,
        "task_generation": context.task.generation,
        "run_id": context.run.run_id,
        "contract_reference": context.contract.as_ref().map(|contract| serde_json::json!({
            "contract_id": contract.contract_id,
            "version": contract.version,
        })),
        "source": context.task.provenance,
        "destination": destination,
        "admission_policy": admission_policy_name(binding.admission_policy()),
        "result_route": result_route_fence,
        "result_policy": result_policy,
    }))
}

pub(super) const fn admission_policy_name(policy: CapabilityAdmissionPolicy) -> &'static str {
    match policy {
        CapabilityAdmissionPolicy::Direct => "direct",
        CapabilityAdmissionPolicy::PolicyAuthorizedDirect => "policy_authorized_direct",
        CapabilityAdmissionPolicy::ReviewerMayApprove => "reviewer_may_approve",
        CapabilityAdmissionPolicy::AlwaysAsk => "always_ask",
    }
}

async fn observed_fetch_arguments(
    store: &noema_store::NoemaStore,
    payload: &serde_json::Value,
) -> Result<Option<serde_json::Value>, noema_store::StoreError> {
    let Ok(request) = noema_capabilities::web::fetch::parse_arguments(payload) else {
        return Ok(None);
    };
    let Ok(normalized) = noema_capabilities::web::url_policy::normalize_observed_url(&request.url)
    else {
        return Ok(None);
    };
    if !store.has_observed_url(&normalized).await? {
        return Ok(None);
    }
    let mut arguments = serde_json::json!({
        "url": normalized,
        "max_chars": request.max_chars,
    });
    if let Some(reason) = request.reason {
        arguments["reason"] = serde_json::Value::String(reason);
    }
    let normalized = if payload.get("arguments").is_some() {
        serde_json::json!({"arguments": arguments})
    } else {
        arguments
    };
    Ok(Some(normalized))
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
        CapabilityError::AuthenticationRequired { .. } => "authentication_required",
        CapabilityError::Failed => "failed",
        CapabilityError::OutcomeUncertain => "outcome_uncertain",
    }
}
