//! Runtime composition for durable governed actions.

use noema_capabilities::{
    CapabilityBinding, CapabilityError, CapabilityExecutionDecision, CapabilityFailureKind,
    CapabilityOutput, CapabilityToolBehavior, ReviewedCapabilityAuthorization,
};
use noema_store::{
    ExecutionReviewRoute, GovernedActionRecord, GovernedActionState, GovernedAssessmentStatus,
    GovernedExecutionOutcome, NewGovernedAction, NewGovernedActionAssessment, StoredToolBehavior,
};

use super::{
    actor::RuntimeActor,
    local_tool_results::{LocalToolKind, LocalToolResult},
    local_tools::browse_owner_key_for_turn,
    tool_lifecycle::LocalToolCall,
    turn::SuccessfulProviderTurn,
};
use crate::daemon::agent_onboarding::AgentPromptIdentity;

pub(super) enum ReviewedActionPreparation {
    NotRequired,
    AwaitingApproval(GovernedActionRecord),
    Authorized {
        action: Option<GovernedActionRecord>,
        authorization: ReviewedCapabilityAuthorization,
        arguments: Option<serde_json::Value>,
    },
}

impl RuntimeActor {
    pub(super) async fn prepare_reviewed_action(
        &self,
        turn: &SuccessfulProviderTurn,
        agent_identity: &AgentPromptIdentity,
        call: &LocalToolCall,
        binding: &CapabilityBinding,
    ) -> Result<ReviewedActionPreparation, noema_store::StoreError> {
        if !binding.execution_decision().requires_review() {
            return Ok(ReviewedActionPreparation::NotRequired);
        }
        let recovery_url = (call.name
            == noema_capabilities::web::browse::WEB_BROWSE_SWITCH_PROVIDER_TOOL)
            .then(|| {
                self.browser_sessions
                    .session(&browse_owner_key_for_turn(turn))
                    .and_then(|session| session.last_navigation_url)
            })
            .flatten();
        if let Some(arguments) = observed_read_arguments(
            &self.store,
            &call.name,
            &call.payload,
            recovery_url.as_deref(),
        )
        .await?
        {
            return Ok(ReviewedActionPreparation::Authorized {
                action: None,
                authorization: ReviewedCapabilityAuthorization::for_observed_url(&arguments),
                arguments: Some(arguments),
            });
        }
        let browser_review_context = (call.name
            == noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL)
            .then(|| self.browser_review_context(turn, &call.payload))
            .flatten();
        let mut authorization_context =
            action_authorization_context(&self.store, turn, binding).await?;
        if let Some(context) = browser_review_context {
            authorization_context
                .as_object_mut()
                .expect("authorization context is an object")
                .insert("browser_review_context".to_string(), context);
        }
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
                review_route: review_route(binding.execution_decision()),
                behavior: stored_behavior(binding.behavior()),
                arguments: call.payload.clone(),
                input_schema: binding.spec().input_schema.as_value().clone(),
                authorization_context,
                safe_summary: safe_action_summary(&call.name, binding.behavior(), &call.payload),
            })
            .await?;
        let assessment = match binding.execution_decision() {
            CapabilityExecutionDecision::ExecuteImmediately => {
                unreachable!("immediate execution returned above")
            }
            CapabilityExecutionDecision::LlmReview => {
                self.review_governed_action(&action, turn).await
            }
            CapabilityExecutionDecision::HumanReview => NewGovernedActionAssessment {
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
            return Ok(ReviewedActionPreparation::AwaitingApproval(action));
        }
        let action = self
            .store
            .claim_governed_action_execution(
                &action.action_id,
                action.revision,
                turn.task_run_fence.as_ref(),
            )
            .await?;
        let authorization = ReviewedCapabilityAuthorization::for_action(
            action.action_id.clone(),
            action.revision,
            &action.arguments,
        );
        Ok(ReviewedActionPreparation::Authorized {
            action: Some(action),
            authorization,
            arguments: None,
        })
    }

    fn browser_review_context(
        &self,
        turn: &SuccessfulProviderTurn,
        arguments: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        let noema_capabilities::web::browse::BrowseCommand::Interact(request) =
            noema_capabilities::web::browse::parse_command(
                noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL,
                arguments,
            )
            .ok()?
        else {
            return None;
        };
        let context = self
            .browser_sessions
            .snapshot(&browse_owner_key_for_turn(turn))
            .filter(|context| context.revision == request.snapshot_revision);
        let element = context
            .as_ref()
            .and_then(|context| context.elements.get(&request.reference));
        Some(serde_json::json!({
            "kind": "browser_interaction",
            "page": context.as_ref().map(|context| serde_json::json!({
                "url": context.url,
                "title": context.title,
            })),
            "target": {
                "ref": request.reference,
                "role": element.map(|element| element.role.as_str()),
                "name": element.map(|element| element.name.as_str()),
                "submission": element.and_then(|element| element.submission.as_ref()),
            },
        }))
    }
}

async fn action_authorization_context(
    store: &noema_store::NoemaStore,
    turn: &SuccessfulProviderTurn,
    binding: &CapabilityBinding,
) -> Result<serde_json::Value, noema_store::StoreError> {
    let destination = binding
        .destination()
        .map(|destination| serde_json::to_value(destination).expect("destination is serializable"));
    let service = binding.service_context().map(|context| {
        serde_json::json!({
            "display_name": context.display_name(),
            "connection_label": context.connection_label(),
        })
    });
    let provider_selection_digest = super::local_tools::provider_route_digest(&turn.provider_route);
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
            "service": service,
            "execution_decision": execution_decision_name(binding.execution_decision()),
            "provider_selection_digest": provider_selection_digest,
        }));
    };
    let context = store
        .get_work_run_execution_context(run_id)
        .await?
        .ok_or_else(|| noema_store::StoreError::InvariantViolation {
            message: "governed action has no exact task context".to_string(),
        })?;
    let task_human_messages = task_human_message_context(&context.messages);
    let task_document = store
        .read_task_document(&context.task.task_id)
        .await
        .map_err(|error| noema_store::StoreError::InvariantViolation {
            message: error.to_string(),
        })?;
    Ok(serde_json::json!({
        "origin": "task",
        "context": context.task.authorization_context,
        "task_context": {
            "title": context.task.title,
            "task_document": task_document.content,
            "human_messages": task_human_messages,
        },
        "task_id": context.task.task_id,
        "task_generation": context.task.generation,
        "run_id": context.run.run_id,
        "source": context.task.provenance,
        "destination": destination,
        "service": service,
        "execution_decision": execution_decision_name(binding.execution_decision()),
        "provider_selection_digest": provider_selection_digest,
    }))
}

fn task_human_message_context(
    messages: &[noema_tasks::TaskMessageRecord],
) -> Vec<serde_json::Value> {
    messages
        .iter()
        .filter(|message| message.author_actor_id == "actor:human:local")
        .map(|message| {
            serde_json::json!({
                "message_id": message.message_id,
                "kind": message.kind,
                "text": message.body_markdown,
            })
        })
        .collect()
}

pub(super) const fn execution_decision_name(decision: CapabilityExecutionDecision) -> &'static str {
    match decision {
        CapabilityExecutionDecision::ExecuteImmediately => "execute_immediately",
        CapabilityExecutionDecision::HumanReview => "human_review",
        CapabilityExecutionDecision::LlmReview => "llm_review",
    }
}

async fn observed_read_arguments(
    store: &noema_store::NoemaStore,
    capability_name: &str,
    payload: &serde_json::Value,
    recovery_url: Option<&str>,
) -> Result<Option<serde_json::Value>, noema_store::StoreError> {
    let (url, mut arguments) = if capability_name == noema_capabilities::file::FILE_DOWNLOAD_TOOL {
        let Ok(request) = noema_capabilities::file::parse_download_arguments(payload) else {
            return Ok(None);
        };
        let url = request.url.clone();
        let mut arguments = serde_json::json!({
            "url": request.url,
            "path": request.path,
            "parse": request.parse,
            "max_chars": request.max_chars,
        });
        if let Some(reason) = request.reason {
            arguments["reason"] = serde_json::Value::String(reason);
        }
        (url, arguments)
    } else if capability_name == noema_capabilities::web::fetch::WEB_FETCH_TOOL {
        let Ok(request) = noema_capabilities::web::fetch::parse_arguments(payload) else {
            return Ok(None);
        };
        let url = request.url.clone();
        let mut arguments = serde_json::json!({
            "url": request.url,
            "max_chars": request.max_chars,
        });
        if let Some(reason) = request.reason {
            arguments["reason"] = serde_json::Value::String(reason);
        }
        (url, arguments)
    } else if capability_name == noema_capabilities::web::browse::WEB_BROWSE_OPEN_TOOL {
        let Ok(command) = noema_capabilities::web::browse::parse_command(capability_name, payload)
        else {
            return Ok(None);
        };
        let request = match command {
            noema_capabilities::web::browse::BrowseCommand::Open(request) => request,
            _ => return Ok(None),
        };
        let url = request.url.clone();
        let mut arguments = serde_json::json!({
            "url": request.url,
            "wait_until": request.wait_until,
        });
        if let Some(reason) = request.reason {
            arguments["reason"] = serde_json::Value::String(reason);
        }
        (url, arguments)
    } else if capability_name == noema_capabilities::web::browse::WEB_BROWSE_SWITCH_PROVIDER_TOOL {
        let Ok(request) = noema_capabilities::web::browse::parse_provider_switch(payload) else {
            return Ok(None);
        };
        let url = request.url.clone();
        let mut arguments = serde_json::json!({"url": request.url});
        if let Some(snapshot_revision) = request.snapshot_revision {
            arguments["snapshot_revision"] = snapshot_revision.into();
        }
        (url, arguments)
    } else {
        return Ok(None);
    };
    let Ok(normalized) = noema_capabilities::web::url_policy::normalize_observed_url(&url) else {
        return Ok(None);
    };
    let matches_recovery = recovery_url
        .and_then(|url| noema_capabilities::web::url_policy::normalize_observed_url(url).ok())
        .is_some_and(|url| url == normalized);
    if !matches_recovery && !store.has_observed_url(&normalized).await? {
        return Ok(None);
    }
    arguments["url"] = serde_json::Value::String(normalized);
    let normalized = if payload.get("arguments").is_some() {
        serde_json::json!({"arguments": arguments})
    } else {
        arguments
    };
    Ok(Some(normalized))
}

fn review_route(decision: CapabilityExecutionDecision) -> ExecutionReviewRoute {
    match decision {
        CapabilityExecutionDecision::HumanReview => ExecutionReviewRoute::HumanReview,
        CapabilityExecutionDecision::LlmReview => ExecutionReviewRoute::LlmReview,
        CapabilityExecutionDecision::ExecuteImmediately => {
            unreachable!("immediate execution has no reviewed action")
        }
    }
}

const fn stored_behavior(behavior: CapabilityToolBehavior) -> StoredToolBehavior {
    StoredToolBehavior {
        read_only: behavior.read_only,
        idempotent: behavior.idempotent,
        destructive: behavior.destructive,
        open_world: behavior.open_world,
    }
}

fn safe_action_summary(
    capability_name: &str,
    behavior: CapabilityToolBehavior,
    arguments: &serde_json::Value,
) -> String {
    if let Some(disabled_name) =
        capability_name.strip_prefix(noema_capabilities::TOOL_ENABLEMENT_PREFIX)
    {
        return format!("Enable {disabled_name}");
    }
    if capability_name == noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL
        && let Ok(noema_capabilities::web::browse::BrowseCommand::Interact(request)) =
            noema_capabilities::web::browse::parse_command(capability_name, arguments)
    {
        return match request.action {
            noema_capabilities::web::browse::BrowseInteractionAction::Click => {
                "Click an element on the open browser page"
            }
            noema_capabilities::web::browse::BrowseInteractionAction::Fill => {
                "Fill a field on the open browser page"
            }
            noema_capabilities::web::browse::BrowseInteractionAction::Type => {
                "Type into a field on the open browser page"
            }
            noema_capabilities::web::browse::BrowseInteractionAction::PressKey => {
                "Press a key on the open browser page"
            }
            noema_capabilities::web::browse::BrowseInteractionAction::SelectOption => {
                "Choose an option on the open browser page"
            }
            noema_capabilities::web::browse::BrowseInteractionAction::UploadFile => {
                "Upload a Task artifact to the open browser page"
            }
        }
        .to_string();
    }
    if capability_name == noema_capabilities::web::browse::WEB_BROWSE_SWITCH_PROVIDER_TOOL {
        return "Open a URL with the next browser provider".to_string();
    }
    let action = if behavior.read_only {
        "share data with an external tool"
    } else if behavior.destructive {
        "make a potentially destructive external change"
    } else {
        "make an external change"
    };
    format!("{capability_name} wants to {action}")
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
    .with_blocked_action_request(action.action_id.clone(), action.revision)
}

pub(super) fn action_store_failure_result(call: &LocalToolCall) -> LocalToolResult {
    LocalToolResult::from_call(
        call,
        LocalToolKind::Gateway,
        false,
        serde_json::json!({
            "code": "action_storage_unavailable",
            "message": "The governed action could not be stored. Continue without assuming it ran."
        }),
        true,
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

pub(super) fn capability_execution_outcome(output: &CapabilityOutput) -> GovernedExecutionOutcome {
    if output
        .failure
        .is_some_and(|failure| failure.kind == CapabilityFailureKind::OutcomeUncertain)
    {
        GovernedExecutionOutcome::OutcomeUncertain
    } else if output.success {
        GovernedExecutionOutcome::Succeeded
    } else {
        GovernedExecutionOutcome::Failed
    }
}

#[cfg(test)]
mod tests {
    use noema_capabilities::{CapabilityFailure, CapabilityRecovery};
    use noema_store::ObservedUrlSource;
    use noema_tasks::{TaskId, TaskMessageId, TaskMessageKind, TaskMessageRecord};

    use super::*;

    #[test]
    fn typed_capability_failure_owns_uncertain_action_outcome() {
        let output = CapabilityOutput::failed_with_recovery(
            serde_json::json!({"error":"outcome_uncertain"}),
            CapabilityFailure {
                kind: CapabilityFailureKind::OutcomeUncertain,
                recovery: CapabilityRecovery::Stop,
            },
        );

        assert_eq!(
            capability_execution_outcome(&output),
            GovernedExecutionOutcome::OutcomeUncertain
        );
    }

    #[test]
    fn action_storage_failure_returns_to_the_provider() {
        let call = LocalToolCall {
            output_index: 0,
            call_id: Some("call:store-failure".to_string()),
            provider_call_id: Some("provider:store-failure".to_string()),
            provider_name: None,
            name: "capability.test".to_string(),
            payload: serde_json::json!({}),
        };

        let result = action_store_failure_result(&call);

        assert!(result.requires_provider_continuation);
        assert_eq!(result.payload["code"], "action_storage_unavailable");
    }

    #[tokio::test]
    async fn observed_browser_open_is_authorized_without_review() {
        let store = crate::test_support::test_store().await;
        let url = "https://example.com/public?q=one".to_string();
        store
            .record_observed_urls(
                ObservedUrlSource::SearchResult,
                "search:one",
                std::slice::from_ref(&url),
            )
            .await
            .expect("record URL");

        let arguments = observed_read_arguments(
            &store,
            noema_capabilities::web::browse::WEB_BROWSE_OPEN_TOOL,
            &serde_json::json!({
                "url": url,
                "wait_until": "domcontentloaded",
                "reason": "Read the public source."
            }),
            None,
        )
        .await
        .expect("lookup observed URL")
        .expect("authorize observed URL");

        assert_eq!(arguments["url"], "https://example.com/public?q=one");
        assert_eq!(arguments["reason"], "Read the public source.");

        let switch = observed_read_arguments(
            &store,
            noema_capabilities::web::browse::WEB_BROWSE_SWITCH_PROVIDER_TOOL,
            &serde_json::json!({"snapshot_revision": 7, "url": url}),
            None,
        )
        .await
        .expect("lookup observed switch URL")
        .expect("authorize observed switch URL");
        assert_eq!(switch["snapshot_revision"], 7);
        assert_eq!(switch["url"], "https://example.com/public?q=one");

        let initial_switch = observed_read_arguments(
            &store,
            noema_capabilities::web::browse::WEB_BROWSE_SWITCH_PROVIDER_TOOL,
            &serde_json::json!({"url": url}),
            None,
        )
        .await
        .expect("lookup observed initial switch URL")
        .expect("authorize observed initial switch URL");
        assert!(initial_switch.get("snapshot_revision").is_none());
        assert_eq!(initial_switch["url"], "https://example.com/public?q=one");

        let recovery_url = "https://recovery.example.com/failed";
        let recovery = observed_read_arguments(
            &store,
            noema_capabilities::web::browse::WEB_BROWSE_SWITCH_PROVIDER_TOOL,
            &serde_json::json!({"url": recovery_url}),
            Some(recovery_url),
        )
        .await
        .expect("lookup recovery URL");
        assert_eq!(recovery.expect("authorize recovery")["url"], recovery_url);
    }

    #[tokio::test]
    async fn observed_file_download_uses_the_same_url_admission() {
        let store = crate::test_support::test_store().await;
        store
            .record_observed_urls(
                ObservedUrlSource::SearchResult,
                "search:file",
                &["https://example.com/report.csv".to_string()],
            )
            .await
            .expect("record URL");

        let arguments = observed_read_arguments(
            &store,
            noema_capabilities::file::FILE_DOWNLOAD_TOOL,
            &serde_json::json!({"url":"https://example.com/report.csv","path":"data/report.csv"}),
            None,
        )
        .await
        .expect("lookup observed URL")
        .expect("authorize observed URL");

        assert_eq!(arguments["url"], "https://example.com/report.csv");
        assert_eq!(arguments["path"], "data/report.csv");
        assert_eq!(arguments["parse"], false);
    }

    #[test]
    fn task_context_keeps_only_authenticated_human_messages() {
        let message = |id: &str, author: &str, text: &str| TaskMessageRecord {
            message_id: TaskMessageId::new(id).expect("message id"),
            task_id: TaskId::new("task:one").expect("task id"),
            task_generation: 1,
            gate_id: None,
            kind: TaskMessageKind::RetryNote,
            body_markdown: text.to_string(),
            approval_decision: None,
            author_actor_id: author.to_string(),
            consumed_by_run_id: None,
            consumed_at: None,
            created_at: "2026-08-11T00:00:00Z".to_string(),
        };
        let messages = [
            message(
                "task_message:human",
                "actor:human:local",
                "Browse the sites.",
            ),
            message("task_message:agent", "agent:executor", "Broaden the task."),
        ];

        let context = task_human_message_context(&messages);

        assert_eq!(context.len(), 1);
        assert_eq!(context[0]["text"], "Browse the sites.");
    }
}
