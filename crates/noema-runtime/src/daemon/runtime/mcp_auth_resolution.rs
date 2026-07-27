//! Exact resumption of MCP calls after interactive authentication.

use std::{collections::HashSet, sync::Arc};

use noema_capabilities::{
    CapabilityError, CapabilityInvoker, CapabilityRegistryRouter, CapabilityRouter,
    GovernedCapabilityAdmission,
};
use noema_conversations::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem, ReplayMode,
};
use noema_store::{
    CapabilityAuthenticationRequestRecord, CapabilityAuthenticationRequestState,
    GovernedActionState, GovernedExecutionOutcome, WorkCommandService,
};

use super::{action_gateway::capability_failure_code, actor::RuntimeActor};
use crate::daemon::{
    ConversationRuntimeEvent, RuntimeError, TurnActivityStatus, TurnStreamEvent, TurnTranscriptItem,
};

impl RuntimeActor {
    pub(super) async fn recover_capability_authentication_origins(
        &mut self,
    ) -> Result<(), RuntimeError> {
        let retained = self
            .store
            .list_active_capability_authentication_argument_references()
            .await?
            .into_iter()
            .collect::<HashSet<_>>();
        self.capability_auth_arguments
            .remove_unreferenced(&retained)
            .map_err(|error| RuntimeError::Protocol(error.to_string()))?;
        let interrupted = self
            .store
            .list_interrupted_capability_authentication_resumptions()
            .await?;
        for request in interrupted {
            self.recover_interrupted_authentication_resumption(&request)
                .await?;
        }
        self.publish_capability_authentication_origins().await
    }

    pub(super) async fn publish_capability_authentication_origins(
        &mut self,
    ) -> Result<(), RuntimeError> {
        let requests = self
            .store
            .list_unpublished_capability_authentication_origins()
            .await?;
        for request in requests {
            self.publish_one_authentication_origin(&request).await?;
        }
        Ok(())
    }

    pub(super) async fn resume_mcp_authentication_attempt(
        &mut self,
        attempt_id: &str,
    ) -> Result<(), RuntimeError> {
        let requests = self
            .store
            .list_capability_authentication_requests_for_attempt(attempt_id)
            .await?;
        let mut first_error = None;
        for pending in requests {
            let request = match self
                .store
                .claim_capability_authentication_resumption(&pending.request_id, pending.revision)
                .await
            {
                Ok(request) => request,
                Err(error) => {
                    first_error.get_or_insert(error.into());
                    continue;
                }
            };
            if let Err(error) = self.resume_mcp_authentication_request(request).await {
                let recovery = self
                    .recover_interrupted_authentication_resumption(&pending)
                    .await;
                if let Err(recovery) = recovery {
                    first_error.get_or_insert(recovery);
                } else {
                    first_error.get_or_insert(error);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub(super) async fn skip_mcp_authentication_request(
        &mut self,
        request_id: &str,
        revision: u64,
        human_id: &str,
    ) -> Result<CapabilityAuthenticationRequestRecord, RuntimeError> {
        let request = self
            .store
            .cancel_capability_authentication_request(request_id, revision, human_id)
            .await?;
        self.finish_governed_action_for_auth(
            &request,
            GovernedExecutionOutcome::Failed,
            None,
            "authentication_skipped",
        )
        .await?;
        self.publish_and_finalize_authentication_origin(&request)
            .await?;
        Ok(request)
    }

    async fn resume_mcp_authentication_request(
        &mut self,
        request: CapabilityAuthenticationRequestRecord,
    ) -> Result<(), RuntimeError> {
        let arguments = self
            .capability_auth_arguments
            .load(&request.protected_arguments_ref, &request.arguments_sha256)
            .ok();
        let catalog =
            self.capability_bindings.catalog().await.map_err(|_| {
                RuntimeError::Protocol("capability catalog is unavailable".to_string())
            })?;
        let binding_is_available = !catalog.availability_notices.iter().any(|notice| {
            notice
                .capability
                .as_ref()
                .is_some_and(|capability| capability.as_str() == request.capability_name)
        });
        let binding = catalog.snapshot.resolve(&request.capability_name);
        let governed_is_current =
            if let Some((action_id, revision)) = request.governed_action.as_ref() {
                self.store
                    .get_governed_action(action_id, *revision)
                    .await?
                    .is_some_and(|action| {
                        action.state == GovernedActionState::AwaitingAuthentication
                            && action.owner_human_id == request.owner_human_id
                            && action.capability_name == request.capability_name
                            && action.operation_token == request.operation_token
                            && action.arguments_sha256 == request.arguments_sha256
                    })
            } else {
                true
            };
        let origin_is_current = if let Some(run_id) = request.run_id.as_deref() {
            self.store
                .get_work_run_execution_context(run_id)
                .await
                .ok()
                .flatten()
                .is_some_and(|context| {
                    request.task_generation == Some(context.task.generation)
                        && request.requesting_agent_id == context.run.agent_id
                        && context.run.status == noema_tasks::RunStatus::WaitingForApproval
                })
        } else {
            true
        };
        let digest = GovernedCapabilityAdmission {
            action_id: request.request_id.clone(),
            revision: request.revision,
            arguments_sha256: request.arguments_sha256.clone(),
        };
        let arguments_are_current = arguments
            .as_ref()
            .is_some_and(|arguments| digest.matches_arguments(arguments));
        let binding_is_current = binding.is_some_and(|binding| {
            let destination_matches = binding.destination().is_some_and(|destination| {
                request.challenge.matches_destination(
                    destination.service_id(),
                    destination.connection_id(),
                    destination.revision(),
                )
            });
            destination_matches
                && binding.target().operation_token().as_str() == request.operation_token
                && binding.spec().input_schema.as_value() == &request.input_schema
        });
        let current_route = if let Some(run_id) = request.run_id.as_deref() {
            match self.store.get_work_run_execution_context(run_id).await? {
                Some(context) => self
                    .resolve_static_provider_route(context.run.model)
                    .await
                    .ok(),
                None => None,
            }
        } else {
            self.resolve_primary_provider().await.ok()
        };
        let result_context_is_current = match (binding, current_route.as_ref()) {
            (Some(binding), Some(route)) => {
                let route_matches =
                    super::capability_result_projection::CapabilityResultRoute::matches_digest(
                        route,
                        &request.provider_selection_digest,
                    ) && request
                        .result_context
                        .get("provider_selection_digest")
                        .and_then(serde_json::Value::as_str)
                        == Some(request.provider_selection_digest.as_str());
                let result_policy = serde_json::to_value(binding.result_policy())
                    .expect("capability result policy is serializable");
                let destination = binding
                    .destination()
                    .and_then(|destination| serde_json::to_value(destination).ok());
                route_matches
                    && request.result_context.get("result_policy") == Some(&result_policy)
                    && request.result_context.get("destination") == destination.as_ref()
            }
            _ => false,
        };
        let valid = governed_is_current
            && origin_is_current
            && arguments_are_current
            && binding_is_current
            && result_context_is_current;
        if !valid {
            let output = superseded_authentication_output(
                governed_is_current
                    && origin_is_current
                    && arguments_are_current
                    && binding_is_available
                    && binding.is_some()
                    && result_context_is_current,
            );
            let request = self
                .store
                .finish_capability_authentication_request(
                    &request.request_id,
                    request.revision,
                    CapabilityAuthenticationRequestState::Superseded,
                    Some(&output),
                    Some("capability_changed"),
                )
                .await?;
            self.finish_governed_action_for_auth(
                &request,
                GovernedExecutionOutcome::Failed,
                None,
                "capability_changed",
            )
            .await?;
            self.publish_and_finalize_authentication_origin(&request)
                .await?;
            return Ok(());
        }

        let arguments = arguments.expect("validated protected arguments are present");

        let router =
            CapabilityRegistryRouter::new(self.capability_invokers.iter().map(|registration| {
                (
                    registration.key().clone(),
                    registration.invoker().clone() as Arc<dyn CapabilityInvoker + '_>,
                )
            }))
            .expect("runtime capability invoker keys are unique");
        let dispatch = if let Some((action_id, revision)) = request.governed_action.as_ref() {
            router
                .dispatch_governed(
                    catalog.snapshot,
                    request.capability_name.clone(),
                    arguments.clone(),
                    GovernedCapabilityAdmission {
                        action_id: action_id.clone(),
                        revision: *revision,
                        arguments_sha256: request.arguments_sha256.clone(),
                    },
                )
                .await
        } else {
            router
                .dispatch(catalog.snapshot, request.capability_name.clone(), arguments)
                .await
        };

        let (success, payload, failure_code, action_outcome) = match dispatch {
            Ok(dispatch) => {
                let success = dispatch.output.success;
                (
                    success,
                    dispatch.persisted.output.unwrap_or_else(
                        || serde_json::json!({"result": "omitted_by_persistence_policy"}),
                    ),
                    (!success).then_some("tool_declared_failure"),
                    if success {
                        GovernedExecutionOutcome::Succeeded
                    } else {
                        GovernedExecutionOutcome::Failed
                    },
                )
            }
            Err(failure)
                if matches!(
                    &failure.error,
                    CapabilityError::AuthenticationRequired { challenge }
                        if challenge == &request.challenge
                ) =>
            {
                self.store
                    .retry_capability_authentication_request(&request.request_id, request.revision)
                    .await?;
                return Ok(());
            }
            Err(failure) => {
                let code = capability_failure_code(&failure.error);
                let outcome = if failure.error == CapabilityError::OutcomeUncertain {
                    GovernedExecutionOutcome::OutcomeUncertain
                } else {
                    GovernedExecutionOutcome::Failed
                };
                (
                    false,
                    failure
                        .persisted
                        .output
                        .unwrap_or_else(|| serde_json::json!({"code": code})),
                    Some(code),
                    outcome,
                )
            }
        };
        self.finish_governed_action_for_auth(
            &request,
            action_outcome,
            Some(&payload),
            failure_code.unwrap_or(""),
        )
        .await?;
        let output = serde_json::json!({"success": success, "payload": payload});
        let request = self
            .store
            .finish_capability_authentication_request(
                &request.request_id,
                request.revision,
                CapabilityAuthenticationRequestState::Completed,
                Some(&output),
                failure_code,
            )
            .await?;
        self.publish_and_finalize_authentication_origin(&request)
            .await?;
        Ok(())
    }

    async fn publish_and_finalize_authentication_origin(
        &mut self,
        request: &CapabilityAuthenticationRequestRecord,
    ) -> Result<(), RuntimeError> {
        self.resume_mcp_request_origin(request, &request.owner_human_id)
            .await?;
        self.store
            .mark_capability_authentication_origin_resumed(&request.request_id, request.revision)
            .await?;
        let _ = self
            .capability_auth_arguments
            .remove(&request.protected_arguments_ref);
        Ok(())
    }

    async fn publish_one_authentication_origin(
        &mut self,
        request: &CapabilityAuthenticationRequestRecord,
    ) -> Result<(), RuntimeError> {
        let success = request
            .output
            .as_ref()
            .and_then(|output| output.get("success"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let payload = request
            .output
            .as_ref()
            .and_then(|output| output.get("payload"));
        let outcome = if success {
            GovernedExecutionOutcome::Succeeded
        } else if request.failure_code.as_deref() == Some("outcome_uncertain") {
            GovernedExecutionOutcome::OutcomeUncertain
        } else {
            GovernedExecutionOutcome::Failed
        };
        self.finish_governed_action_for_auth(
            request,
            outcome,
            payload,
            request
                .failure_code
                .as_deref()
                .unwrap_or("authentication_failed"),
        )
        .await?;
        self.publish_and_finalize_authentication_origin(request)
            .await
    }

    async fn recover_interrupted_authentication_resumption(
        &mut self,
        pending: &CapabilityAuthenticationRequestRecord,
    ) -> Result<(), RuntimeError> {
        let Some(mut request) = self
            .store
            .get_capability_authentication_request(&pending.request_id, pending.revision)
            .await?
        else {
            return Ok(());
        };
        if request.state == CapabilityAuthenticationRequestState::Resuming {
            let action = if let Some((action_id, revision)) = request.governed_action.as_ref() {
                self.store.get_governed_action(action_id, *revision).await?
            } else {
                None
            };
            let recovered = action.as_ref().map(|action| {
                (
                    action.state,
                    action.output.clone(),
                    action.failure_code.clone(),
                )
            });
            let (output, failure_code) = recovered_authentication_output(recovered);
            request = self
                .store
                .finish_capability_authentication_request(
                    &request.request_id,
                    request.revision,
                    CapabilityAuthenticationRequestState::Completed,
                    Some(&output),
                    failure_code.as_deref(),
                )
                .await?;
        }
        if matches!(
            request.state,
            CapabilityAuthenticationRequestState::Completed
                | CapabilityAuthenticationRequestState::Cancelled
                | CapabilityAuthenticationRequestState::Superseded
        ) {
            self.publish_one_authentication_origin(&request).await?;
        }
        Ok(())
    }

    async fn finish_governed_action_for_auth(
        &self,
        request: &CapabilityAuthenticationRequestRecord,
        outcome: GovernedExecutionOutcome,
        output: Option<&serde_json::Value>,
        failure_code: &str,
    ) -> Result<(), RuntimeError> {
        let Some((action_id, revision)) = request.governed_action.as_ref() else {
            return Ok(());
        };
        let Some(action) = self.store.get_governed_action(action_id, *revision).await? else {
            return Ok(());
        };
        match action.state {
            GovernedActionState::AwaitingAuthentication => {
                self.store
                    .resume_governed_action_after_authentication(action_id, *revision)
                    .await?;
            }
            GovernedActionState::Executing => {}
            _ => return Ok(()),
        }
        self.store
            .finish_governed_action_execution(
                action_id,
                *revision,
                outcome,
                output,
                (outcome != GovernedExecutionOutcome::Succeeded).then_some(failure_code),
            )
            .await?;
        Ok(())
    }

    async fn resume_mcp_request_origin(
        &mut self,
        request: &CapabilityAuthenticationRequestRecord,
        human_id: &str,
    ) -> Result<(), RuntimeError> {
        if request.task_id.is_some() {
            let continuation_run_id =
                WorkCommandService::new(self.store.clone(), self.provider_registry.clone())
                    .resume_after_capability_authentication(
                        &request.request_id,
                        request.revision,
                        &format!("actor:{human_id}"),
                    )
                    .await?;
            self.publish_intervention_task_changed(
                request.task_id.as_deref().expect("task origin checked"),
                continuation_run_id,
            );
            return Ok(());
        }
        self.publish_foreground_mcp_outcome(request).await
    }

    async fn publish_foreground_mcp_outcome(
        &mut self,
        request: &CapabilityAuthenticationRequestRecord,
    ) -> Result<(), RuntimeError> {
        let conversation_id = request.conversation_id.as_ref().ok_or_else(|| {
            RuntimeError::Protocol("MCP authentication origin is unavailable".to_string())
        })?;
        let items = self
            .store
            .list_conversation_items(conversation_id, ReplayMode::Visible)
            .await?;
        let parent_item_id = items.iter().find_map(|item| {
            if item.turn_id.as_deref() != request.turn_id.as_deref() {
                return None;
            }
            if item.kind == ConversationItemKind::ToolCall
                && item
                    .metadata
                    .get("output_index")
                    .and_then(serde_json::Value::as_u64)
                    == Some(request.output_index as u64)
            {
                return Some(item.item_id.clone());
            }
            let action_id = request
                .governed_action
                .as_ref()
                .map(|value| value.0.as_str())?;
            (item.kind == ConversationItemKind::ApprovalRequest
                && item.payload_json.pointer("/metadata/action/id")?.as_str()? == action_id)
                .then(|| item.item_id.clone())
        });
        let output = request.output.clone().unwrap_or_else(|| {
            serde_json::json!({
                "success": false,
                "payload": {"code": request.failure_code.as_deref().unwrap_or("authentication_skipped")}
            })
        });
        let success = output
            .get("success")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let payload = serde_json::json!({
            "result": "omitted_after_delayed_resume",
            "request_id": request.request_id,
            "failure_code": request.failure_code,
        });
        let status = if success {
            ConversationItemStatus::Completed
        } else {
            ConversationItemStatus::Failed
        };
        let activity_status = if success {
            TurnActivityStatus::Completed
        } else {
            TurnActivityStatus::Failed
        };
        let summary = if success {
            "Tool completed after sign-in"
        } else {
            "Tool was not completed"
        };
        let activity_id = format!("mcp_authentication:{}", request.request_id);
        let metadata = serde_json::json!({
            "action": {
                "call_id": request.call_id,
                "provider_call_id": request.provider_call_id,
                "provider_name": request.provider_name,
                "name": request.capability_name,
                "success": success,
                "payload": payload,
            },
            "mcp_authentication": {
                "request_id": request.request_id,
                "revision": request.revision,
                "state": request.state.as_str(),
                "failure_code": request.failure_code,
            },
        });
        let payload_json = serde_json::json!({
            "id": activity_id,
            "activity_kind": "tool_result",
            "status": activity_status,
            "title": format!("Tool result: {}", request.capability_name),
            "summary": summary,
            "metadata": metadata,
        });
        let (item, inserted) = self
            .store
            .append_conversation_item_with_id_if_absent(
                format!("item:mcp_authentication:{}", request.request_id),
                NewConversationItem {
                    conversation_id: conversation_id.clone(),
                    turn_id: request.turn_id.clone(),
                    parent_item_id,
                    kind: ConversationItemKind::ToolResult,
                    status,
                    author: ActorRef::agent("agent:primary")
                        .expect("static primary agent id is valid"),
                    content_text: Some(summary.to_string()),
                    payload_json,
                    metadata: serde_json::json!({"source": "mcp_authentication"}),
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
                            title: format!("Tool result: {}", request.capability_name),
                            summary: Some(summary.to_string()),
                            metadata,
                        }),
                    }),
                });
        }
        if request.failure_code.as_deref() == Some("outcome_uncertain") {
            self.runtime_events
                .publish_conversation(ConversationRuntimeEvent::Completed {
                    conversation_id: conversation_id.clone(),
                    client_message_id: None,
                });
            return Ok(());
        }
        let (item_tx, mut item_rx) = tokio::sync::mpsc::unbounded_channel();
        let events = self.runtime_events.clone();
        let relay = async move {
            while let Some(event) = item_rx.recv().await {
                events.publish_conversation(ConversationRuntimeEvent::Turn {
                    client_message_id: None,
                    event: Box::new(event),
                });
            }
        };
        let continuation = self.continue_after_human_intervention(
            conversation_id.clone(),
            request.request_id.clone(),
            trigger_item_id,
            item_tx,
        );
        let (result, ()) = tokio::join!(continuation, relay);
        self.runtime_events
            .publish_conversation(ConversationRuntimeEvent::Completed {
                conversation_id: conversation_id.clone(),
                client_message_id: None,
            });
        result
    }
}

fn recovered_authentication_output(
    action: Option<(
        GovernedActionState,
        Option<serde_json::Value>,
        Option<String>,
    )>,
) -> (serde_json::Value, Option<String>) {
    let Some((state, persisted_output, persisted_failure)) = action else {
        return outcome_uncertain_authentication_output();
    };
    match state {
        GovernedActionState::Succeeded => (
            serde_json::json!({
                "success": true,
                "payload": persisted_output.unwrap_or_else(|| {
                    serde_json::json!({"result": "omitted_by_persistence_policy"})
                })
            }),
            None,
        ),
        GovernedActionState::Failed => {
            let code = persisted_failure.unwrap_or_else(|| "failed".to_string());
            (
                serde_json::json!({
                    "success": false,
                    "payload": persisted_output.unwrap_or_else(|| serde_json::json!({"code": &code}))
                }),
                Some(code),
            )
        }
        GovernedActionState::OutcomeUncertain => outcome_uncertain_authentication_output(),
        _ => outcome_uncertain_authentication_output(),
    }
}

fn outcome_uncertain_authentication_output() -> (serde_json::Value, Option<String>) {
    (
        serde_json::json!({"success": false, "payload": {"code": "outcome_uncertain"}}),
        Some("outcome_uncertain".to_string()),
    )
}

fn superseded_authentication_output(retry_current_capability: bool) -> serde_json::Value {
    let mut payload = serde_json::json!({"code": "capability_changed"});
    if retry_current_capability {
        payload["retry_with_current_capability"] = serde_json::Value::Bool(true);
        payload["guidance"] = serde_json::Value::String(
            "Authentication completed, but this tool's definition changed. Retry it as a new call using the current tool definition and rebuild any arguments that no longer validate."
                .to_string(),
        );
    }
    serde_json::json!({"success": false, "payload": payload})
}

#[cfg(test)]
#[path = "mcp_auth_resolution/tests.rs"]
mod tests;
