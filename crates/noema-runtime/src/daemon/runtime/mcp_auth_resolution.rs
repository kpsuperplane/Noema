//! Exact resumption of MCP calls after interactive authentication.

use std::sync::Arc;

use noema_capabilities::{
    CapabilityError, CapabilityInvoker, CapabilityRegistryRouter, CapabilityRouter,
    GovernedCapabilityAdmission,
};
use noema_conversations::{
    ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem, ReplayMode,
};
use noema_store::{
    GovernedActionState, GovernedExecutionOutcome, McpAuthenticationRequestRecord,
    McpAuthenticationRequestState, WorkCommandService,
};

use super::{action_gateway::capability_failure_code, actor::RuntimeActor};
use crate::daemon::{
    ConversationRuntimeEvent, RuntimeError, TurnActivityStatus, TurnStreamEvent, TurnTranscriptItem,
};

impl RuntimeActor {
    pub(super) async fn recover_interrupted_mcp_authentication_resumptions(
        &mut self,
    ) -> Result<(), RuntimeError> {
        let requests = self
            .store
            .list_interrupted_mcp_authentication_resumptions()
            .await?;
        for request in requests {
            let request = self
                .store
                .finish_mcp_authentication_request(
                    &request.request_id,
                    request.revision,
                    McpAuthenticationRequestState::Superseded,
                    Some(&serde_json::json!({
                        "success": false,
                        "payload": {"code": "outcome_unknown_after_restart"}
                    })),
                    Some("outcome_unknown_after_restart"),
                )
                .await?;
            self.finish_governed_action_for_auth(
                &request,
                false,
                None,
                "outcome_unknown_after_restart",
            )
            .await?;
            self.resume_mcp_request_origin(&request, &request.owner_human_id)
                .await?;
        }
        Ok(())
    }

    pub(super) async fn resume_mcp_authentication_attempt(
        &mut self,
        attempt_id: &str,
    ) -> Result<(), RuntimeError> {
        let requests = self
            .store
            .list_mcp_authentication_requests_for_attempt(attempt_id)
            .await?;
        for request in requests {
            self.resume_mcp_authentication_request(request).await?;
        }
        Ok(())
    }

    pub(super) async fn skip_mcp_authentication_request(
        &mut self,
        request_id: &str,
        revision: u64,
        human_id: &str,
    ) -> Result<McpAuthenticationRequestRecord, RuntimeError> {
        let request = self
            .store
            .cancel_mcp_authentication_request(request_id, revision, human_id)
            .await?;
        self.finish_governed_action_for_auth(&request, false, None, "authentication_skipped")
            .await?;
        self.resume_mcp_request_origin(&request, human_id).await?;
        Ok(request)
    }

    async fn resume_mcp_authentication_request(
        &mut self,
        pending: McpAuthenticationRequestRecord,
    ) -> Result<(), RuntimeError> {
        let request = self
            .store
            .claim_mcp_authentication_resumption(&pending.request_id, pending.revision)
            .await?;
        let catalog = self
            .capability_bindings
            .catalog()
            .await
            .map_err(|_| RuntimeError::Protocol("capability catalog is unavailable".to_string()))?
            .snapshot;
        let binding = catalog.resolve(&request.capability_name);
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
        let valid = origin_is_current
            && binding.is_some_and(|binding| {
                binding.target().operation_token().as_str() == request.operation_token
                    && binding.spec().input_schema.as_value() == &request.input_schema
                    && digest.matches_arguments(&request.arguments)
            });
        if !valid {
            let request = self
                .store
                .finish_mcp_authentication_request(
                    &request.request_id,
                    request.revision,
                    McpAuthenticationRequestState::Superseded,
                    Some(&serde_json::json!({
                        "success": false,
                        "payload": {"code": "capability_changed"}
                    })),
                    Some("capability_changed"),
                )
                .await?;
            self.finish_governed_action_for_auth(&request, false, None, "capability_changed")
                .await?;
            self.resume_mcp_request_origin(&request, &request.owner_human_id)
                .await?;
            return Ok(());
        }

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
                    catalog,
                    request.capability_name.clone(),
                    request.arguments.clone(),
                    GovernedCapabilityAdmission {
                        action_id: action_id.clone(),
                        revision: *revision,
                        arguments_sha256: request.arguments_sha256.clone(),
                    },
                )
                .await
        } else {
            router
                .dispatch(
                    catalog,
                    request.capability_name.clone(),
                    request.arguments.clone(),
                )
                .await
        };

        let (success, payload, failure_code) = match dispatch {
            Ok(dispatch) => (
                dispatch.output.success,
                dispatch.output.payload,
                (!dispatch.output.success).then_some("tool_declared_failure"),
            ),
            Err(failure)
                if matches!(
                    failure.error,
                    CapabilityError::AuthenticationRequired { .. }
                ) =>
            {
                self.store
                    .retry_mcp_authentication_request(&request.request_id, request.revision)
                    .await?;
                return Ok(());
            }
            Err(failure) => {
                let code = capability_failure_code(&failure.error);
                (false, serde_json::json!({"code": code}), Some(code))
            }
        };
        self.finish_governed_action_for_auth(
            &request,
            success,
            Some(&payload),
            failure_code.unwrap_or(""),
        )
        .await?;
        let output = serde_json::json!({"success": success, "payload": payload});
        let request = self
            .store
            .finish_mcp_authentication_request(
                &request.request_id,
                request.revision,
                McpAuthenticationRequestState::Completed,
                Some(&output),
                failure_code,
            )
            .await?;
        self.resume_mcp_request_origin(&request, &request.owner_human_id)
            .await
    }

    async fn finish_governed_action_for_auth(
        &self,
        request: &McpAuthenticationRequestRecord,
        success: bool,
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
                if success {
                    GovernedExecutionOutcome::Succeeded
                } else {
                    GovernedExecutionOutcome::Failed
                },
                output,
                (!success).then_some(failure_code),
            )
            .await?;
        Ok(())
    }

    async fn resume_mcp_request_origin(
        &mut self,
        request: &McpAuthenticationRequestRecord,
        human_id: &str,
    ) -> Result<(), RuntimeError> {
        if request.task_id.is_some() {
            WorkCommandService::new(self.store.clone(), self.provider_registry.clone())
                .resume_after_mcp_authentication(
                    &request.request_id,
                    request.revision,
                    &format!("actor:{human_id}"),
                )
                .await?;
            return Ok(());
        }
        self.publish_foreground_mcp_outcome(request).await
    }

    async fn publish_foreground_mcp_outcome(
        &mut self,
        request: &McpAuthenticationRequestRecord,
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
        let payload = output.get("payload").cloned().unwrap_or_default();
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
