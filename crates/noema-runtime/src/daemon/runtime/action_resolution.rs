//! Human resolution and exact replay of durable governed actions.

use std::{collections::HashMap, sync::Arc};

use noema_capabilities::{
    CapabilityError, CapabilityInvoker, CapabilityRegistryRouter, PayloadSanitizer,
    ReviewedCapabilityAuthorization,
};
use noema_conversations::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem,
};
use noema_providers::{WebBrowseBackendHandle, WebBrowseOwner};
use noema_store::{
    GovernedActionDecision, GovernedActionRecord, GovernedActionState, GovernedExecutionOutcome,
    NewCapabilityAuthenticationRequest, StoredToolBehavior, WorkCommandService,
};

use super::{
    action_gateway::{capability_failure_code, execution_decision_name},
    actor::RuntimeActor,
};
use crate::daemon::{
    ConversationRuntimeEvent, RuntimeError, TurnActivityStatus, TurnStreamEvent, TurnTranscriptItem,
};

impl RuntimeActor {
    pub(super) async fn recover_governed_action_origins(&mut self) -> Result<(), RuntimeError> {
        for action in self
            .store
            .list_interrupted_governed_action_resumptions()
            .await?
        {
            self.resume_action_task(&action, &action.owner_human_id)
                .await?;
        }
        for action in self
            .store
            .list_pending_governed_actions("human:local", None, None, 100)
            .await?
            .into_iter()
            .filter(is_session_bound_browser_action)
        {
            self.supersede_and_resume(action, "human:local", "browser_session_unavailable")
                .await?;
        }
        Ok(())
    }

    pub(super) async fn resolve_governed_action(
        &mut self,
        action_id: &str,
        revision: u64,
        human_id: &str,
        decision: GovernedActionDecision,
    ) -> Result<GovernedActionRecord, RuntimeError> {
        let current = self
            .store
            .get_governed_action(action_id, revision)
            .await?
            .ok_or_else(|| RuntimeError::Protocol("governed action is unavailable".to_string()))?;
        if current.owner_human_id != human_id {
            return Err(RuntimeError::Protocol(
                "governed action is unavailable".to_string(),
            ));
        }
        if decision == GovernedActionDecision::Approve
            && current.state == GovernedActionState::AwaitingApproval
            && is_session_bound_browser_action(&current)
            && !self.browser_session_available(&current).await
        {
            return self
                .supersede_and_resume(current, human_id, "browser_session_unavailable")
                .await;
        }
        let action = match (current.state, decision) {
            (GovernedActionState::AwaitingApproval, _) => {
                self.store
                    .decide_governed_action(action_id, revision, human_id, decision)
                    .await?
            }
            (GovernedActionState::Executable, GovernedActionDecision::Approve) => current,
            (GovernedActionState::AwaitingAuthentication, GovernedActionDecision::Approve) => {
                return Ok(current);
            }
            (
                GovernedActionState::Succeeded
                | GovernedActionState::Failed
                | GovernedActionState::OutcomeUncertain
                | GovernedActionState::Declined
                | GovernedActionState::Superseded
                | GovernedActionState::Cancelled,
                _,
            ) => {
                self.resume_action_task(&current, human_id).await?;
                return Ok(current);
            }
            _ => {
                return Err(RuntimeError::Protocol(
                    "governed action decision is stale".to_string(),
                ));
            }
        };
        if decision == GovernedActionDecision::Decline {
            self.resume_action_task(&action, human_id).await?;
            return Ok(action);
        }
        if action.capability_name == "acp.permission" {
            let option_id = action
                .arguments
                .get("allow_once_option_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    RuntimeError::Protocol("ACP permission has no allow-once option".to_string())
                })?;
            let claimed = self
                .store
                .claim_governed_action_execution(action_id, revision, None)
                .await?;
            let finished = self
                .store
                .finish_governed_action_execution(
                    action_id,
                    revision,
                    GovernedExecutionOutcome::Succeeded,
                    Some(&serde_json::json!({
                        "decision": "allow_once",
                        "option_id": option_id,
                        "request_fingerprint": claimed.arguments_sha256,
                    })),
                    None,
                )
                .await?;
            self.resume_action_task(&finished, human_id).await?;
            return Ok(finished);
        }

        let authorization = ReviewedCapabilityAuthorization::for_action(
            action.action_id.clone(),
            action.revision,
            &action.arguments,
        );
        if authorization.arguments_sha256 != action.arguments_sha256 {
            return self
                .supersede_and_resume(action, human_id, "payload_digest_changed")
                .await;
        }

        let web_spec = match action.capability_name.as_str() {
            noema_capabilities::web::search::WEB_SEARCH_TOOL => {
                noema_capabilities::web::search::tool_spec().map(Some)
            }
            noema_capabilities::web::fetch::WEB_FETCH_TOOL => {
                noema_capabilities::web::fetch::tool_spec().map(Some)
            }
            name if name.starts_with("web.browse.") => {
                noema_capabilities::web::browse::tool_specs()
                    .map(|specs| specs.into_iter().find(|spec| spec.name.as_str() == name))
            }
            _ => Ok(None),
        }
        .map_err(|_| RuntimeError::Protocol("web capability schema is unavailable".to_string()))?;
        let (binding, catalog) = if let Some(spec) = web_spec {
            let binding = super::model_tools::native_web_binding(&self.store, spec)
                .await
                .map_err(|_| RuntimeError::Protocol("web capability is unavailable".to_string()))?;
            (binding, None)
        } else {
            let catalog = self
                .capability_bindings
                .catalog()
                .await
                .map_err(|_| {
                    RuntimeError::Protocol("capability catalog is unavailable".to_string())
                })?
                .snapshot;
            let Some(binding) = catalog.resolve(&action.capability_name).cloned() else {
                return self
                    .supersede_and_resume(action, human_id, "capability_removed")
                    .await;
            };
            (binding, Some(catalog))
        };
        let current_destination = binding
            .destination()
            .and_then(|destination| serde_json::to_value(destination).ok());
        if action.authorization_context.get("destination") != current_destination.as_ref() {
            return self
                .supersede_and_resume(action, human_id, "destination_changed")
                .await;
        }
        if action.authorization_context.get("execution_decision")
            != Some(&serde_json::json!(execution_decision_name(
                binding.execution_decision()
            )))
        {
            return self
                .supersede_and_resume(action, human_id, "execution_decision_changed")
                .await;
        }
        let behavior = binding.behavior();
        let current_behavior = StoredToolBehavior {
            read_only: behavior.read_only,
            idempotent: behavior.idempotent,
            destructive: behavior.destructive,
            open_world: behavior.open_world,
        };
        if binding.target().operation_token().as_str() != action.operation_token
            || Some(current_behavior) != action.behavior
            || binding.spec().input_schema.as_value() != &action.input_schema
        {
            return self
                .supersede_and_resume(action, human_id, "capability_changed")
                .await;
        }

        let claimed = self
            .store
            .claim_governed_action_execution(action_id, revision, None)
            .await?;
        if let Some(output) = self.execute_approved_web_action(&claimed).await {
            let outcome_uncertain = claimed.capability_name.starts_with("web.browse.")
                && output
                    .payload
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    == Some("browser action outcome is uncertain");
            let outcome = if outcome_uncertain {
                GovernedExecutionOutcome::OutcomeUncertain
            } else if output.success {
                GovernedExecutionOutcome::Succeeded
            } else {
                GovernedExecutionOutcome::Failed
            };
            let persisted_output = match claimed.capability_name.as_str() {
                noema_capabilities::web::fetch::WEB_FETCH_TOOL => {
                    noema_capabilities::WebFetchPayloadSanitizer.persist_output(&output.payload)
                }
                name if name.starts_with("web.browse.") => {
                    noema_capabilities::WebBrowsePayloadSanitizer.persist_output(&output.payload)
                }
                _ => noema_capabilities::RedactingPayloadSanitizer.persist_output(&output.payload),
            };
            let finished = self
                .store
                .finish_governed_action_execution(
                    action_id,
                    revision,
                    outcome,
                    persisted_output.as_ref(),
                    if outcome_uncertain {
                        Some("outcome_uncertain")
                    } else {
                        (!output.success).then_some("tool_declared_failure")
                    },
                )
                .await?;
            self.resume_action_task(&finished, human_id).await?;
            return Ok(finished);
        }
        let router =
            CapabilityRegistryRouter::new(self.capability_invokers.iter().map(|registration| {
                (
                    registration.key().clone(),
                    registration.invoker().clone() as Arc<dyn CapabilityInvoker + '_>,
                )
            }))
            .expect("runtime capability invoker keys are unique");
        let dispatch = router
            .dispatch_reviewed(
                catalog.expect("non-web governed action has a live catalog"),
                claimed.capability_name.clone(),
                claimed.arguments.clone(),
                authorization,
            )
            .await;
        let finished = match dispatch {
            Ok(dispatch) => {
                let outcome = if dispatch.output.success {
                    GovernedExecutionOutcome::Succeeded
                } else {
                    GovernedExecutionOutcome::Failed
                };
                self.store
                    .finish_governed_action_execution(
                        action_id,
                        revision,
                        outcome,
                        dispatch.persisted.output.as_ref(),
                        (!dispatch.output.success).then_some("tool_declared_failure"),
                    )
                    .await?
            }
            Err(failure) => {
                if let CapabilityError::AuthenticationRequired { challenge } = &failure.error {
                    let destination = claimed.authorization_context.get("destination");
                    let challenge_matches_destination = destination.is_some_and(|destination| {
                        challenge.matches_destination(
                            destination
                                .get("service_id")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default(),
                            destination
                                .get("connection_id")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default(),
                            destination
                                .get("revision")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default(),
                        )
                    });
                    let protected = challenge_matches_destination
                        .then(|| self.capability_auth_arguments.persist(&claimed.arguments))
                        .transpose();
                    let route_digest = claimed
                        .authorization_context
                        .get("provider_selection_digest")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned);
                    let (Ok(Some(protected)), Some(route_digest)) = (protected, route_digest)
                    else {
                        let finished = self
                            .store
                            .finish_governed_action_execution(
                                action_id,
                                revision,
                                GovernedExecutionOutcome::Failed,
                                failure.persisted.output.as_ref(),
                                Some("authentication_challenge_invalid"),
                            )
                            .await
                            .map_err(RuntimeError::from)?;
                        self.resume_action_task(&finished, human_id).await?;
                        return Ok(finished);
                    };
                    let request = self
                        .store
                        .create_capability_authentication_request(
                            NewCapabilityAuthenticationRequest {
                                owner_human_id: human_id.to_string(),
                                conversation_id: claimed.conversation_id.clone(),
                                turn_id: claimed.turn_id.clone(),
                                task_id: claimed.task_id.clone(),
                                run_id: claimed.run_id.clone(),
                                task_generation: claimed
                                    .authorization_context
                                    .get("task_generation")
                                    .and_then(serde_json::Value::as_u64),
                                requesting_agent_id: claimed.requesting_agent_id.clone(),
                                challenge: challenge.clone(),
                                capability_name: claimed.capability_name.clone(),
                                operation_token: claimed.operation_token.clone(),
                                input_schema: claimed.input_schema.clone(),
                                protected_arguments_ref: protected.reference.clone(),
                                arguments_sha256: protected.sha256.clone(),
                                provider_selection_digest: route_digest.clone(),
                                output_index: 0,
                                call_id: None,
                                provider_call_id: None,
                                provider_name: None,
                                governed_action: Some((
                                    claimed.action_id.clone(),
                                    claimed.revision,
                                )),
                                result_context: serde_json::json!({
                                    "provider_selection_digest": route_digest,
                                    "destination": claimed.authorization_context.get("destination"),
                                }),
                            },
                            None,
                        )
                        .await;
                    if let Ok(request) = &request {
                        if request.protected_arguments_ref != protected.reference {
                            let _ = self.capability_auth_arguments.remove(&protected.reference);
                        }
                        return self
                            .store
                            .get_governed_action(action_id, revision)
                            .await?
                            .ok_or_else(|| {
                                RuntimeError::Protocol(
                                    "authentication-paused action disappeared".to_string(),
                                )
                            });
                    }
                    let _ = self.capability_auth_arguments.remove(&protected.reference);
                }
                let outcome = if failure.error == CapabilityError::OutcomeUncertain {
                    GovernedExecutionOutcome::OutcomeUncertain
                } else {
                    GovernedExecutionOutcome::Failed
                };
                self.store
                    .finish_governed_action_execution(
                        action_id,
                        revision,
                        outcome,
                        failure.persisted.output.as_ref(),
                        Some(capability_failure_code(&failure.error)),
                    )
                    .await?
            }
        };
        self.resume_action_task(&finished, human_id).await?;
        Ok(finished)
    }

    pub(super) async fn browser_action_session_availability(
        &self,
        actions: Vec<(String, u64)>,
        human_id: &str,
    ) -> Result<HashMap<String, bool>, RuntimeError> {
        let backend = self.web_browse_runtime_provider_resolution().await.ok();
        let mut availability = HashMap::with_capacity(actions.len());
        for (action_id, revision) in actions {
            let Some(action) = self.store.get_governed_action(&action_id, revision).await? else {
                continue;
            };
            if action.owner_human_id != human_id || !is_session_bound_browser_action(&action) {
                continue;
            }
            availability.insert(
                action_id,
                browser_session_available(backend.as_ref(), &action).await,
            );
        }
        Ok(availability)
    }

    async fn browser_session_available(&self, action: &GovernedActionRecord) -> bool {
        let Ok(backend) = self.web_browse_runtime_provider_resolution().await else {
            return false;
        };
        browser_session_available(Some(&backend), action).await
    }

    async fn supersede_and_resume(
        &mut self,
        action: GovernedActionRecord,
        human_id: &str,
        reason: &str,
    ) -> Result<GovernedActionRecord, RuntimeError> {
        let action = self
            .store
            .supersede_governed_action(&action.action_id, action.revision, reason)
            .await?;
        self.resume_action_task(&action, human_id).await?;
        Ok(action)
    }

    pub(super) async fn resume_action_task(
        &mut self,
        action: &GovernedActionRecord,
        human_id: &str,
    ) -> Result<(), RuntimeError> {
        if action.task_id.is_none() {
            return self.publish_foreground_action_outcome(action).await;
        }
        let actor_id = format!("actor:{human_id}");
        let continuation_run_id =
            WorkCommandService::new(self.store.clone(), self.provider_registry.clone())
                .resume_after_governed_action(&action.action_id, action.revision, &actor_id)
                .await?;
        self.publish_intervention_task_changed(
            action.task_id.as_deref().expect("task origin checked"),
            continuation_run_id,
        );
        Ok(())
    }

    pub(super) fn publish_intervention_task_changed(&self, task_id: &str, run_id: Option<String>) {
        self.runtime_events
            .publish_task(crate::daemon::TaskRuntimeEvent::Changed {
                task_id: task_id.to_string(),
                run_id,
            });
        self.runtime_events
            .publish_work(crate::daemon::WorkRuntimeEvent::Committed {
                workspace_id: "workspace:personal".to_string(),
                task_id: Some(task_id.to_string()),
            });
    }

    async fn publish_foreground_action_outcome(
        &mut self,
        action: &GovernedActionRecord,
    ) -> Result<(), RuntimeError> {
        let Some(conversation_id) = action.conversation_id.as_ref() else {
            return Err(RuntimeError::Protocol(
                "foreground action has no conversation origin".to_string(),
            ));
        };
        let request = self
            .store
            .get_action_request_source(&action.action_id, action.revision)
            .await?
            .and_then(action_request_context);
        let Some(request) = request else {
            crate::daemon::log_system_error(
                &self.system_errors,
                crate::daemon::SYSTEM_ERROR_RUNTIME_INVARIANT,
                "action request continuation source is unavailable",
                Some(serde_json::json!({
                    "action_id": action.action_id,
                    "conversation_id": conversation_id,
                })),
                RuntimeError::Protocol("action request approval item is unavailable".to_string()),
            );
            return Ok(());
        };
        let (status, activity_status, summary) = action_display_state(action.state);
        let success = action.state == GovernedActionState::Succeeded;
        let activity_id = format!("governed_action:{}:{}", action.action_id, action.revision);
        let tool_payload = serde_json::json!({
            "status": action.state.as_str(),
            "action_id": action.action_id,
            "failure_code": action.failure_code,
            "result": action.output,
        });
        let metadata = serde_json::json!({
            "action": {
                "call_id": request.call_id,
                "provider_call_id": request.provider_call_id,
                "provider_name": request.provider_name,
                "name": request.name,
                "success": success,
                "payload": tool_payload,
            },
            "governed_action": {
                "action_id": action.action_id,
                "revision": action.revision,
                "state": action.state.as_str(),
                "failure_code": action.failure_code,
            },
        });
        let payload_json = serde_json::json!({
            "id": activity_id,
            "activity_kind": "tool_result",
            "status": activity_status,
            "title": format!("Tool result: {}", request.name),
            "summary": summary,
            "metadata": metadata,
        });
        let (item, inserted) = self
            .store
            .append_conversation_item_with_id_if_absent(
                format!(
                    "item:governed_action:{}:{}",
                    action.action_id, action.revision
                ),
                NewConversationItem {
                    conversation_id: conversation_id.clone(),
                    turn_id: action.turn_id.clone(),
                    parent_item_id: Some(request.item_id),
                    kind: ConversationItemKind::ToolResult,
                    status,
                    author: ActorRef::new("agent:primary")
                        .expect("static primary agent id is valid"),
                    content_text: Some(summary.to_string()),
                    payload_json,
                    metadata: serde_json::json!({"source": "governed_action"}),
                },
            )
            .await?;
        let trigger_item_id = item.item_id.clone();
        if inserted {
            self.runtime_events
                .publish_conversation(ConversationRuntimeEvent::Turn {
                    client_message_id: None,
                    event: Box::new(TurnStreamEvent::ConversationItem {
                        conversation_id: item.conversation_id,
                        item_id: item.item_id,
                        cursor: Some(item.cursor),
                        turn_id: item.turn_id,
                        metadata: item.metadata,
                        item: Box::new(TurnTranscriptItem::Activity {
                            id: activity_id,
                            activity_kind: "tool_result".to_string(),
                            status: activity_status,
                            title: format!("Tool result: {}", request.name),
                            summary: Some(summary.to_string()),
                            metadata,
                        }),
                    }),
                });
        }
        if action.state == GovernedActionState::OutcomeUncertain {
            self.runtime_events
                .publish_conversation(ConversationRuntimeEvent::Completed {
                    conversation_id: conversation_id.clone(),
                    client_message_id: None,
                });
            return Ok(());
        }
        let (item_tx, mut item_rx) = tokio::sync::mpsc::unbounded_channel();
        let relay_events = self.runtime_events.clone();
        let relay = async move {
            while let Some(event) = item_rx.recv().await {
                relay_events.publish_conversation(ConversationRuntimeEvent::Turn {
                    client_message_id: None,
                    event: Box::new(event),
                });
            }
        };
        let continuation = self.continue_after_governed_action(
            conversation_id.clone(),
            action.action_id.clone(),
            trigger_item_id,
            item_tx,
        );
        let (result, ()) = tokio::join!(continuation, relay);
        self.runtime_events
            .publish_conversation(ConversationRuntimeEvent::Completed {
                conversation_id: conversation_id.clone(),
                client_message_id: None,
            });
        if let Err(error) = result {
            crate::daemon::log_system_error(
                &self.system_errors,
                crate::daemon::SYSTEM_ERROR_RUNTIME_INVARIANT,
                "governed action continuation failed",
                Some(serde_json::json!({
                    "action_id": action.action_id,
                    "conversation_id": conversation_id,
                })),
                error,
            );
        }
        Ok(())
    }
}

fn is_session_bound_browser_action(action: &GovernedActionRecord) -> bool {
    matches!(
        action.capability_name.as_str(),
        noema_capabilities::web::browse::WEB_BROWSE_NAVIGATE_TOOL
            | noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL
            | noema_capabilities::web::browse::WEB_BROWSE_HISTORY_TOOL
    )
}

async fn browser_session_available(
    backend: Option<&WebBrowseBackendHandle>,
    action: &GovernedActionRecord,
) -> bool {
    let (Some(backend), Some(owner)) = (
        backend,
        super::local_tools::browse_owner_key_for_action(action),
    ) else {
        return false;
    };
    backend.has_session(&WebBrowseOwner::new(owner)).await
}

struct ApprovalRequestContext {
    item_id: String,
    call_id: Option<String>,
    provider_call_id: Option<String>,
    provider_name: Option<String>,
    name: String,
}

fn action_request_context(
    item: noema_conversations::ConversationItemRecord,
) -> Option<ApprovalRequestContext> {
    if item.kind != ConversationItemKind::ApprovalRequest {
        return None;
    }
    let payload = item.payload_json.pointer("/metadata/action/payload")?;
    Some(ApprovalRequestContext {
        item_id: item.item_id,
        call_id: optional_string(payload, "call_id"),
        provider_call_id: optional_string(payload, "provider_call_id"),
        provider_name: optional_string(payload, "provider_name"),
        name: optional_string(payload, "name")?,
    })
}

fn optional_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn action_display_state(
    state: GovernedActionState,
) -> (ConversationItemStatus, TurnActivityStatus, &'static str) {
    match state {
        GovernedActionState::Succeeded => (
            ConversationItemStatus::Completed,
            TurnActivityStatus::Completed,
            "Approved action completed",
        ),
        GovernedActionState::Declined => (
            ConversationItemStatus::Cancelled,
            TurnActivityStatus::Failed,
            "Action declined",
        ),
        GovernedActionState::Failed => (
            ConversationItemStatus::Failed,
            TurnActivityStatus::Failed,
            "Approved action failed",
        ),
        GovernedActionState::OutcomeUncertain => (
            ConversationItemStatus::Interrupted,
            TurnActivityStatus::Failed,
            "Action outcome is uncertain",
        ),
        GovernedActionState::Superseded | GovernedActionState::Cancelled => (
            ConversationItemStatus::Cancelled,
            TurnActivityStatus::Failed,
            "Action is no longer valid",
        ),
        GovernedActionState::Proposed
        | GovernedActionState::AwaitingApproval
        | GovernedActionState::Executable
        | GovernedActionState::Executing
        | GovernedActionState::AwaitingAuthentication => (
            ConversationItemStatus::Failed,
            TurnActivityStatus::Failed,
            "Action resolution is incomplete",
        ),
    }
}
