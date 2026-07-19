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
    ) -> Result<GovernedActionPreparation, noema_store::StoreError> {
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
        let trusted_authority = trusted_action_authority(&self.store, turn, &call.name).await?;
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
                trusted_authority,
                safe_summary: safe_action_summary(&call.name, effect),
            })
            .await?;
        let assessment = self.review_governed_action(&action, turn).await;
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
        let admission = GovernedCapabilityAdmission {
            action_id: action.action_id.clone(),
            revision: action.revision,
            arguments_sha256: action.arguments_sha256.clone(),
        };
        Ok(GovernedActionPreparation::Admitted {
            action: Some(action),
            admission,
            arguments: None,
        })
    }
}

async fn trusted_action_authority(
    store: &noema_store::NoemaStore,
    turn: &SuccessfulProviderTurn,
    capability_name: &str,
) -> Result<serde_json::Value, noema_store::StoreError> {
    let destination = web_destination(store, capability_name).await;
    let Some(run_id) = turn.task_run_id.as_deref() else {
        return Ok(serde_json::json!({
            "origin": "primary_conversation",
            "human_request": turn.user_input,
            "conversation_id": turn.conversation_id,
            "destination": destination,
        }));
    };
    let context = store
        .get_work_run_execution_context(run_id)
        .await?
        .ok_or_else(|| noema_store::StoreError::InvariantViolation {
            message: "governed action has no exact task context".to_string(),
        })?;
    let source = &context.task.provenance;
    let human_request = match source.source_kind {
        noema_tasks::TaskSourceKind::ChatCapture | noema_tasks::TaskSourceKind::ChatDelegate => {
            if let Some(item_id) = source.item_id.as_deref() {
                store
                    .get_visible_conversation_item(item_id)
                    .await?
                    .filter(|item| {
                        item.kind == noema_conversations::ConversationItemKind::UserText
                            && source
                                .conversation_id
                                .as_ref()
                                .is_none_or(|id| &item.conversation_id == id)
                    })
                    .and_then(|item| item.content_text)
            } else {
                None
            }
        }
        noema_tasks::TaskSourceKind::WorkUi
            if source.created_by_actor_id == "actor:human:local" =>
        {
            Some(context.task.description_markdown.clone())
        }
        noema_tasks::TaskSourceKind::WorkUi | noema_tasks::TaskSourceKind::System => None,
    };
    Ok(serde_json::json!({
        "origin": "task",
        "human_request": human_request,
        "task_id": context.task.task_id,
        "task_generation": context.task.generation,
        "run_id": context.run.run_id,
        "contract_reference": context.contract.as_ref().map(|contract| serde_json::json!({
            "contract_id": contract.contract_id,
            "version": contract.version,
        })),
        "source": source,
        "destination": destination,
    }))
}

pub(super) async fn web_destination(
    store: &noema_store::NoemaStore,
    capability_name: &str,
) -> Option<serde_json::Value> {
    let resolved = match capability_name {
        noema_capabilities::web::search::WEB_SEARCH_TOOL => {
            super::web_tools::resolve_web_search_provider(store)
                .await
                .ok()
        }
        noema_capabilities::web::fetch::WEB_FETCH_TOOL => {
            super::web_tools::resolve_web_fetch_provider(store)
                .await
                .ok()
        }
        _ => None,
    }?;
    Some(serde_json::json!({
        "provider_account_id": resolved.provider_account_id,
        "provider_kind": resolved.provider_kind,
        "credential_revision": resolved.credential_revision,
    }))
}

async fn observed_fetch_arguments(
    store: &noema_store::NoemaStore,
    payload: &serde_json::Value,
) -> Result<Option<serde_json::Value>, noema_store::StoreError> {
    let Some(outer) = payload.as_object() else {
        return Ok(None);
    };
    let nested = outer.len() == 1
        && outer
            .get("arguments")
            .and_then(serde_json::Value::as_object)
            .is_some();
    let arguments = if nested {
        outer
            .get("arguments")
            .and_then(serde_json::Value::as_object)
            .expect("nested object checked above")
    } else {
        outer
    };
    if arguments.len() != 1 {
        return Ok(None);
    }
    let Some(url) = arguments.get("url").and_then(serde_json::Value::as_str) else {
        return Ok(None);
    };
    let Ok(normalized) = noema_capabilities::web::url_policy::normalize_observed_url(url) else {
        return Ok(None);
    };
    if !store.has_observed_url(&normalized).await? {
        return Ok(None);
    }
    let normalized = if nested {
        serde_json::json!({"arguments": {"url": normalized}})
    } else {
        serde_json::json!({"url": normalized})
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
        CapabilityError::Failed => "failed",
        CapabilityError::OutcomeUncertain => "outcome_uncertain",
    }
}
