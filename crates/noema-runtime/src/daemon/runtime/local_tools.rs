use crate::agent_execution::{ExecutionRole, ToolPolicy};
use noema_capabilities::{
    CapabilityDispatchFailure, CapabilityError, CapabilityFailureKind, CapabilityFuture,
    CapabilityInvocation, CapabilityInvoker, CapabilityOutput, CapabilityRegistryRouter,
    InvokerKey, PayloadSanitizer, UrlPayloadSanitizer,
};
use noema_providers::ProviderRouteLease;
use noema_store::{GovernedExecutionOutcome, NewCapabilityAuthenticationRequest};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

use super::presentation_tools::{
    PRESENT_A2UI_TOOL, PRESENT_MULTIPLE_CHOICE_TOOL, parse_a2ui_payload,
    parse_multiple_choice_payload,
};
use super::{
    action_gateway::{
        ReviewedActionPreparation, action_store_failure_result, awaiting_approval_result,
        capability_execution_outcome, capability_failure_code,
    },
    actor::{BrowserSessionState, BrowserSnapshotContext, RuntimeActor},
    tool_lifecycle::LocalToolCall,
    turn::SuccessfulProviderTurn,
};
use crate::a2ui::parse_and_reduce;
use crate::daemon::{
    agent_name_tool::{
        AgentNameToolRuntimeContext, execute_update_own_name, is_update_own_name_tool,
    },
    agent_onboarding::AgentPromptIdentity,
    artifact_tool::{
        ArtifactToolRuntimeContext, execute_artifact_create_local_file,
        is_artifact_create_local_file_tool,
    },
    calculation_tool::{execute_calculation, is_calculation_tool},
    task_artifact_tool::{
        TaskArtifactReadContext, execute_task_list_artifacts, execute_task_parse_artifact,
        execute_task_read_artifact, is_task_list_artifacts_tool, is_task_parse_artifact_tool,
        is_task_read_artifact_tool,
    },
    task_tool::{
        TASK_CAPTURE_TOOL, TASK_INSPECT_TOOL, TASK_LIST_TOOL, TaskDelegateRuntimeContext,
        execute_primary_task_tool, execute_scoped_task_capture_tool, execute_scoped_task_file_tool,
        execute_scoped_task_inspect_tool, execute_scoped_task_list_tool, is_primary_task_tool,
        is_task_continue_execution_tool, is_task_file_tool, is_task_finish_execution_tool,
        is_task_finish_planning_tool, is_task_finish_review_tool, is_task_report_blocked_tool,
    },
};
use crate::file_tools::{execute_file_download, execute_file_parse};
use crate::search::tool::is_web_search_tool;
use crate::web_fetch::tool::is_web_fetch_tool;
use crate::{WebBackendRequest, WebBackendResolverError};
use noema_capabilities::web::browse::{
    BrowseCommand, BrowseNavigationRequest, BrowseResponse, BrowseWaitUntil, parse_command,
    parse_provider_switch,
};
use noema_capabilities::web::fetch::WEB_FETCH_TOOL;
use noema_providers::{
    OBSCURA_BROWSER_PROVIDER_ID, ProviderAuthMethod, WebBrowseBackendHandle, WebBrowseError,
    WebBrowseOwner, WebFetchBackendHandle, WebFetchContext, WebFetchError, WebSearchBackendHandle,
};

const PROVIDER_ACCOUNT_UNAUTHENTICATED: &str = "provider account unauthenticated";

pub(super) fn browser_model_visible_payload(mut payload: Value) -> Value {
    if let Some(output) = payload.as_object_mut() {
        output.remove("screenshot");
    }
    payload
}

pub(super) fn provider_route_digest(route: &ProviderRouteLease) -> String {
    let bytes = serde_json::to_vec(&(route.selection(), route.generation()))
        .expect("provider selection metadata is serializable");
    ring::digest::digest(&ring::digest::SHA256, &bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

mod web_actions;

#[path = "native_memory_tools.rs"]
mod native_memory_tools;
use native_memory_tools::execute_native_memory_tool;

pub(super) struct ProviderAuthFailureTarget {
    provider_account_id: String,
    credential_revision: u64,
}

impl RuntimeActor {
    /// Execute a foreground tool through its advertised binding and strict role policy.
    pub(super) async fn execute_local_tool(
        &self,
        turn: &SuccessfulProviderTurn,
        agent_identity: &AgentPromptIdentity,
        call: &LocalToolCall,
    ) -> LocalToolResult {
        self.execute_local_tool_with_policy(
            turn,
            agent_identity,
            call,
            &turn.initial_model_tools.tool_policy,
        )
        .await
    }

    /// Execute one local or injected capability under an explicit role policy.
    pub(super) async fn execute_local_tool_with_policy(
        &self,
        turn: &SuccessfulProviderTurn,
        agent_identity: &AgentPromptIdentity,
        call: &LocalToolCall,
        policy: &ToolPolicy,
    ) -> LocalToolResult {
        let snapshot = &turn.initial_model_tools.bindings;
        let Some(binding) = snapshot.resolve(&call.name).cloned() else {
            if is_builtin_browser_tool(&call.name) {
                return unadvertised_browser_failure_result(call);
            }
            let failure = CapabilityDispatchFailure::from_snapshot(
                snapshot,
                &call.name,
                &call.payload,
                CapabilityError::Denied,
            );
            return gateway_failure_result(call, failure);
        };
        if !policy.allows_tool(&call.name) {
            let failure = CapabilityDispatchFailure::from_snapshot(
                snapshot,
                &call.name,
                &call.payload,
                CapabilityError::Denied,
            );
            return gateway_failure_result(call, failure);
        }
        if let Err(error) = self
            .validate_browser_call(&browse_owner_key_for_turn(turn), &call.name, &call.payload)
            .await
        {
            return browser_validation_failure_result(call, &binding, error);
        }
        let preparation = match self
            .prepare_reviewed_action(turn, agent_identity, call, &binding)
            .await
        {
            Ok(preparation) => preparation,
            Err(_) => return action_store_failure_result(call),
        };
        let reviewed = match preparation {
            ReviewedActionPreparation::NotRequired => None,
            ReviewedActionPreparation::AwaitingApproval(action) => {
                return awaiting_approval_result(call, &action);
            }
            ReviewedActionPreparation::Authorized {
                action,
                authorization,
                arguments,
            } => Some((
                action,
                authorization,
                arguments.unwrap_or_else(|| call.payload.clone()),
            )),
        };

        let runtime_invoker = Arc::new(RuntimeExecutionInvoker::new(
            self,
            turn,
            agent_identity,
            call,
        ));
        let mut invokers = Vec::with_capacity(self.capability_invokers.len() + 1);
        invokers.push((
            InvokerKey::new("runtime-execution"),
            runtime_invoker.clone() as Arc<dyn CapabilityInvoker + '_>,
        ));
        invokers.extend(self.capability_invokers.iter().map(|registration| {
            (
                registration.key().clone(),
                registration.invoker().clone() as Arc<dyn CapabilityInvoker + '_>,
            )
        }));
        let router = CapabilityRegistryRouter::new(invokers)
            .expect("runtime capability invoker keys are unique");
        let dispatch = match reviewed.as_ref() {
            Some((_, authorization, arguments)) => {
                router
                    .dispatch_reviewed(
                        snapshot.clone(),
                        call.name.clone(),
                        arguments.clone(),
                        authorization.clone(),
                    )
                    .await
            }
            None => {
                router
                    .dispatch(snapshot.clone(), call.name.clone(), call.payload.clone())
                    .await
            }
        };
        match dispatch {
            Ok(dispatch) => {
                let outcome_uncertain = dispatch
                    .output
                    .failure
                    .is_some_and(|failure| failure.kind == CapabilityFailureKind::OutcomeUncertain);
                if let Some((Some(action), _, _)) = &reviewed {
                    let outcome = capability_execution_outcome(&dispatch.output);
                    if self
                        .store
                        .finish_governed_action_execution(
                            &action.action_id,
                            action.revision,
                            outcome,
                            dispatch.persisted.output.as_ref(),
                            if outcome_uncertain {
                                Some("outcome_uncertain")
                            } else {
                                (!dispatch.output.success).then_some("tool_declared_failure")
                            },
                        )
                        .await
                        .is_err()
                    {
                        return action_store_failure_result(call);
                    }
                }
                let runtime_result = runtime_invoker.take_result();
                let result = LocalToolResult::from_call(
                    call,
                    runtime_result.map_or(LocalToolKind::Gateway, |result| result.kind),
                    dispatch.output.success,
                    dispatch.output.payload,
                    runtime_result.is_none_or(|result| result.requires_provider_continuation),
                )
                .with_failure(dispatch.output.failure)
                .with_persisted(dispatch.persisted);
                if outcome_uncertain {
                    result.with_blocked_outcome_uncertain()
                } else {
                    result
                }
            }
            Err(failure) => {
                if failure.error == CapabilityError::InvalidArguments
                    && (is_task_finish_planning_tool(&call.name)
                        || is_task_finish_execution_tool(&call.name)
                        || is_task_continue_execution_tool(&call.name)
                        || is_task_finish_review_tool(&call.name)
                        || is_task_report_blocked_tool(&call.name))
                {
                    return LocalToolResult::from_call(
                        call,
                        LocalToolKind::Gateway,
                        false,
                        json!({
                            "code": "invalid_task_terminal",
                            "message": "terminal payload did not match its source input rules",
                        }),
                        true,
                    )
                    .with_persisted(failure.persisted)
                    .with_side_effect(!binding.behavior().read_only);
                }
                if let CapabilityError::AuthenticationRequired { challenge } = &failure.error {
                    let governed_action = reviewed
                        .as_ref()
                        .and_then(|(action, _, _)| action.as_ref())
                        .map(|action| (action.action_id.clone(), action.revision));
                    let arguments = reviewed.as_ref().map_or_else(
                        || call.payload.clone(),
                        |(_, _, arguments)| arguments.clone(),
                    );
                    let challenge_matches_destination =
                        binding.destination().is_some_and(|destination| {
                            challenge.matches_destination(
                                destination.service_id(),
                                destination.connection_id(),
                                destination.revision(),
                            )
                        });
                    let route_digest = provider_route_digest(&turn.provider_route);
                    let protected = challenge_matches_destination
                        .then(|| self.capability_auth_arguments.persist(&arguments))
                        .transpose();
                    if let Ok(Some(protected)) = protected {
                        let request = self
                            .store
                            .create_capability_authentication_request(
                                NewCapabilityAuthenticationRequest {
                                    owner_human_id: "human:local".to_string(),
                                    conversation_id: turn
                                        .task_run_id
                                        .is_none()
                                        .then(|| turn.conversation_id.clone()),
                                    turn_id: turn
                                        .task_run_id
                                        .is_none()
                                        .then(|| turn.turn_id.clone()),
                                    task_id: turn.task_id.clone(),
                                    run_id: turn.task_run_id.clone(),
                                    task_generation: turn
                                        .task_run_fence
                                        .as_ref()
                                        .map(|fence| fence.task_generation),
                                    requesting_agent_id: agent_identity.agent_id.clone(),
                                    challenge: challenge.clone(),
                                    capability_name: call.name.clone(),
                                    operation_token: binding
                                        .target()
                                        .operation_token()
                                        .as_str()
                                        .to_string(),
                                    input_schema: binding.spec().input_schema.as_value().clone(),
                                    protected_arguments_ref: protected.reference.clone(),
                                    arguments_sha256: protected.sha256.clone(),
                                    provider_selection_digest: route_digest.clone(),
                                    output_index: call.output_index,
                                    call_id: call.call_id.clone(),
                                    provider_call_id: call.provider_call_id.clone(),
                                    provider_name: call.provider_name.clone(),
                                    governed_action,
                                    result_context: json!({
                                        "provider_selection_digest": route_digest,
                                        "destination": binding.destination(),
                                    }),
                                },
                                turn.task_run_fence.as_ref(),
                            )
                            .await;
                        if let Ok(request) = &request {
                            if request.protected_arguments_ref != protected.reference {
                                let _ = self.capability_auth_arguments.remove(&protected.reference);
                            }
                            return LocalToolResult::from_call(
                                call,
                                LocalToolKind::Gateway,
                                false,
                                json!({"code": "authentication_required"}),
                                false,
                            )
                            .with_persisted(failure.persisted)
                            .with_blocked_authentication(request.request_id.clone());
                        }
                        let _ = self.capability_auth_arguments.remove(&protected.reference);
                    }
                }
                if let Some((Some(action), _, _)) = &reviewed {
                    let outcome = if failure.error == CapabilityError::OutcomeUncertain {
                        GovernedExecutionOutcome::OutcomeUncertain
                    } else {
                        GovernedExecutionOutcome::Failed
                    };
                    if self
                        .store
                        .finish_governed_action_execution(
                            &action.action_id,
                            action.revision,
                            outcome,
                            failure.persisted.output.as_ref(),
                            Some(capability_failure_code(&failure.error)),
                        )
                        .await
                        .is_err()
                    {
                        return action_store_failure_result(call);
                    }
                }
                gateway_failure_result(call, failure)
            }
        }
        .with_side_effect(!binding.behavior().read_only)
    }

    async fn execute_bound_runtime_tool(
        &self,
        turn: &SuccessfulProviderTurn,
        agent_identity: &AgentPromptIdentity,
        call: &LocalToolCall,
    ) -> Result<LocalToolResult, CapabilityError> {
        let result = if let Some(result) =
            execute_native_memory_tool(self.native_memory.as_ref(), call)
        {
            result
        } else if call.name == PRESENT_MULTIPLE_CHOICE_TOOL {
            match parse_multiple_choice_payload(&call.payload) {
                Ok(_) => LocalToolResult::from_call(
                    call,
                    LocalToolKind::Gateway,
                    false,
                    json!({"error": "presentation call was not intercepted"}),
                    true,
                ),
                Err(message) => LocalToolResult::from_call(
                    call,
                    LocalToolKind::Gateway,
                    false,
                    json!({"error": message}),
                    true,
                ),
            }
        } else if call.name == PRESENT_A2UI_TOOL {
            match parse_a2ui_payload(&call.payload) {
                Err(message) => LocalToolResult::from_call(
                    call,
                    LocalToolKind::Gateway,
                    false,
                    json!({"status": "VALIDATION_FAILED", "message": message}),
                    true,
                ),
                Ok(arguments) => match parse_and_reduce(&turn.conversation_id, &arguments.jsonl) {
                    Err(repair) => LocalToolResult::from_call(
                        call,
                        LocalToolKind::Gateway,
                        false,
                        serde_json::to_value(repair).unwrap_or_else(|_| {
                            json!({"status": "VALIDATION_FAILED", "message": "A2UI validation failed"})
                        }),
                        true,
                    ),
                    Ok(batch) if batch.surfaces.values().any(|surface| !surface.actions.is_empty()) =>
                        LocalToolResult::from_call(
                            call,
                            LocalToolKind::Gateway,
                            false,
                            json!({"error": "action-bearing A2UI call was not intercepted"}),
                            true,
                        ),
                    Ok(batch) => match self.persist_action_free_a2ui(turn, call, &batch).await {
                        Ok(payload) => LocalToolResult::from_call(
                            call,
                            LocalToolKind::Gateway,
                            true,
                            payload,
                            true,
                        ),
                        Err(error) => LocalToolResult::from_call(
                            call,
                            LocalToolKind::Gateway,
                            false,
                            json!({"error": error.to_string()}),
                            true,
                        ),
                    },
                },
            }
        } else if is_update_own_name_tool(&call.name) {
            let context = AgentNameToolRuntimeContext {
                agent_id: agent_identity.agent_id.clone(),
            };
            let result =
                execute_update_own_name(&self.store, &context, call.call_id.clone(), &call.payload)
                    .await;
            LocalToolResult::from_call(
                call,
                LocalToolKind::AgentName,
                result.success,
                result.payload,
                true,
            )
        } else if is_calculation_tool(&call.name) {
            let result = execute_calculation(&call.payload);
            let (success, payload) = match result {
                Ok(payload) => (true, payload),
                Err(error) => (false, json!({"error": error})),
            };
            LocalToolResult::from_call(call, LocalToolKind::Gateway, success, payload, true)
        } else if is_artifact_create_local_file_tool(&call.name) {
            let context = ArtifactToolRuntimeContext {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                user_item_id: turn.user_item_id.clone(),
                created_by_actor_id: agent_identity.agent_id.clone(),
                task_id: turn.task_id.clone(),
                task_run_id: turn.task_run_id.clone(),
            };
            let result = execute_artifact_create_local_file(
                &self.store,
                &self.artifact_operations,
                &context,
                call.call_id.clone(),
                &call.payload,
            )
            .await;
            LocalToolResult::from_call(
                call,
                LocalToolKind::Artifact,
                result.success,
                result.payload,
                true,
            )
        } else if is_task_list_artifacts_tool(&call.name)
            || is_task_parse_artifact_tool(&call.name)
            || is_task_read_artifact_tool(&call.name)
        {
            let context =
                turn.task_id
                    .as_ref()
                    .zip(turn.task_run_id.as_ref())
                    .map(|(task_id, run_id)| TaskArtifactReadContext {
                        task_id: task_id.clone(),
                        run_id: run_id.clone(),
                    });
            let result = if let Some(context) = context {
                if is_task_list_artifacts_tool(&call.name) {
                    execute_task_list_artifacts(&self.store, &context, &call.payload).await
                } else if is_task_parse_artifact_tool(&call.name) {
                    execute_task_parse_artifact(
                        &self.store,
                        &self.artifact_operations,
                        &context,
                        &call.payload,
                    )
                    .await
                } else {
                    execute_task_read_artifact(
                        &self.store,
                        &self.artifact_operations,
                        &context,
                        &call.payload,
                    )
                    .await
                }
            } else {
                Err("task artifact context is unavailable".to_string())
            };
            let (success, payload) = match result {
                Ok(payload) => (true, payload),
                Err(error) => (false, json!({"error": error})),
            };
            LocalToolResult::from_call(call, LocalToolKind::Gateway, success, payload, true)
        } else if call.name == noema_capabilities::file::FILE_PARSE_TOOL {
            let result = execute_file_parse(
                &self.store,
                turn.task_id.as_deref(),
                turn.cwd.as_deref(),
                &call.payload,
            )
            .await;
            let (success, payload) = match result {
                Ok(payload) => (true, payload),
                Err(error) => (false, json!({"error": error})),
            };
            LocalToolResult::from_call(call, LocalToolKind::File, success, payload, true)
        } else if call.name == noema_capabilities::file::FILE_DOWNLOAD_TOOL {
            let result = execute_file_download(
                &self.store,
                turn.task_id.as_deref(),
                turn.cwd.as_deref(),
                &call.payload,
            )
            .await;
            let (success, payload) = match result {
                Ok(payload) => (true, payload),
                Err(error) => (false, json!({"error": error})),
            };
            LocalToolResult::from_call(call, LocalToolKind::File, success, payload, true)
        } else if is_task_file_tool(&call.name) && turn.task_run_id.is_some() {
            let result = match turn.task_id.as_deref() {
                Some(task_id) => {
                    execute_scoped_task_file_tool(
                        &self.store,
                        task_id,
                        turn.initial_model_tools.tool_policy.role(),
                        &call.name,
                        call.call_id.clone(),
                        &call.payload,
                    )
                    .await
                }
                None => crate::daemon::task_tool::TaskToolResult {
                    call_id: call.call_id.clone(),
                    name: call.name.clone(),
                    success: false,
                    payload: json!({"code": "invalid_context", "message": "Task file context is unavailable"}),
                },
            };
            LocalToolResult::from_call(
                call,
                LocalToolKind::Gateway,
                result.success,
                result.payload,
                true,
            )
        } else if matches!(call.name.as_str(), TASK_LIST_TOOL | TASK_INSPECT_TOOL)
            && turn.task_run_id.is_some()
        {
            let result = match turn.task_id.as_deref() {
                Some(task_id) if call.name == TASK_LIST_TOOL => {
                    execute_scoped_task_list_tool(
                        &self.store,
                        task_id,
                        call.call_id.clone(),
                        &call.payload,
                    )
                    .await
                }
                Some(task_id) => {
                    execute_scoped_task_inspect_tool(
                        &self.store,
                        task_id,
                        call.call_id.clone(),
                        &call.payload,
                    )
                    .await
                }
                None => crate::daemon::task_tool::TaskToolResult {
                    call_id: call.call_id.clone(),
                    name: call.name.clone(),
                    success: false,
                    payload: json!({"code": "invalid_context", "message": "background task context is unavailable"}),
                },
            };
            LocalToolResult::from_call(
                call,
                LocalToolKind::Gateway,
                result.success,
                result.payload,
                true,
            )
        } else if call.name == TASK_CAPTURE_TOOL && turn.task_run_id.is_some() {
            let result = match turn.task_id.as_deref() {
                Some(task_id) => {
                    execute_scoped_task_capture_tool(
                        &self.store,
                        &self.provider_registry,
                        &TaskDelegateRuntimeContext {
                            conversation_id: turn.conversation_id.clone(),
                            turn_id: turn.turn_id.clone(),
                            user_item_id: turn.user_item_id.clone(),
                            agent_id: agent_identity.agent_id.clone(),
                            workspace_id: "workspace:personal".to_string(),
                            owner_human_id: "human:local".to_string(),
                            client_time_zone: turn.runtime_environment.timezone.clone(),
                        },
                        task_id,
                        call.call_id.clone(),
                        &call.payload,
                    )
                    .await
                }
                None => crate::daemon::task_tool::TaskToolResult {
                    call_id: call.call_id.clone(),
                    name: call.name.clone(),
                    success: false,
                    payload: json!({"code": "invalid_context", "message": "background task context is unavailable"}),
                },
            };
            publish_captured_task(&self.runtime_events, &result);
            LocalToolResult::from_call(
                call,
                LocalToolKind::Gateway,
                result.success,
                result.payload,
                true,
            )
        } else if is_primary_task_tool(&call.name) {
            let result = execute_primary_task_tool(
                &self.store,
                &self.provider_registry,
                &TaskDelegateRuntimeContext {
                    conversation_id: turn.conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: turn.user_item_id.clone(),
                    agent_id: agent_identity.agent_id.clone(),
                    workspace_id: "workspace:personal".to_string(),
                    owner_human_id: "human:local".to_string(),
                    client_time_zone: turn.runtime_environment.timezone.clone(),
                },
                &call.name,
                call.call_id.clone(),
                &call.payload,
            )
            .await;
            publish_captured_task(&self.runtime_events, &result);
            LocalToolResult::from_call(
                call,
                LocalToolKind::Gateway,
                result.success,
                result.payload,
                true,
            )
        } else if is_task_finish_planning_tool(&call.name)
            || is_task_finish_execution_tool(&call.name)
            || is_task_continue_execution_tool(&call.name)
            || is_task_finish_review_tool(&call.name)
            || is_task_report_blocked_tool(&call.name)
        {
            match crate::daemon::task_run_context::validate_task_terminal(
                turn.initial_model_tools.tool_policy.role(),
                &call.name,
                &call.payload,
            ) {
                Err(message) => LocalToolResult::from_call(
                    call,
                    LocalToolKind::Gateway,
                    false,
                    json!({"code": "invalid_task_terminal", "message": message}),
                    true,
                ),
                Ok(()) => LocalToolResult::from_call(
                    call,
                    LocalToolKind::Gateway,
                    true,
                    call.payload.clone(),
                    false,
                ),
            }
        } else if is_web_search_tool(&call.name) {
            let source = call.call_id.as_deref().unwrap_or(&turn.turn_id);
            let output = self
                .execute_web_search_action(call.call_id.clone(), &call.payload, source)
                .await;
            LocalToolResult::from_call(
                call,
                LocalToolKind::WebSearch,
                output.success,
                output.payload,
                true,
            )
        } else if is_web_fetch_tool(&call.name) {
            let generation_priority = match turn.initial_model_tools.tool_policy.role() {
                ExecutionRole::PrimaryConversation => {
                    noema_providers::GenerationPriority::Foreground
                }
                ExecutionRole::TaskPlanner
                | ExecutionRole::TaskExecutor
                | ExecutionRole::TaskReviewer => noema_providers::GenerationPriority::Background,
            };
            let source = call.call_id.as_deref().unwrap_or(&turn.turn_id);
            let output = self
                .execute_web_fetch_action(
                    generation_priority,
                    call.call_id.clone(),
                    &call.payload,
                    source,
                )
                .await;
            LocalToolResult::from_call(
                call,
                LocalToolKind::WebFetch,
                output.success,
                output.payload,
                true,
            )
        } else if call.name.starts_with("web.browse.") {
            let owner_key = browse_owner_key_for_turn(turn);
            let source = call.call_id.as_deref().unwrap_or(&turn.turn_id);
            let output = self
                .execute_web_browse_action(owner_key, &call.name, &call.payload, source)
                .await;
            let persisted_output_source = output.persisted_output_source().clone();
            let outcome_uncertain = output
                .failure
                .is_some_and(|failure| failure.kind == CapabilityFailureKind::OutcomeUncertain);
            let result = LocalToolResult::from_call(
                call,
                LocalToolKind::WebBrowse,
                output.success,
                output.payload,
                !outcome_uncertain,
            )
            .with_failure(output.failure)
            .with_persisted_output_source(persisted_output_source);
            if outcome_uncertain {
                result.with_blocked_outcome_uncertain()
            } else {
                result
            }
        } else {
            return Err(CapabilityError::UnknownOperation);
        };
        Ok(result)
    }

    async fn web_fetch_runtime_context(
        &self,
        generation_priority: noema_providers::GenerationPriority,
    ) -> Result<WebFetchContext, String> {
        let summarizer_route = Arc::new(self.web_summary_provider.resolve_route().await.map_err(
            |_| "web.fetch summarizer provider is not available in this daemon".to_string(),
        )?);
        let selection = summarizer_route.selection();
        let summarizer_model = selection.model_profile.clone().ok_or_else(|| {
            "web.fetch summarizer selection has no concrete model profile".to_string()
        })?;
        let summarizer_reasoning_effort = selection.reasoning_effort;
        let summarizer_fast_mode = selection.fast_mode;
        Ok(WebFetchContext {
            summarizer_route,
            summarizer_model,
            summarizer_reasoning_effort,
            summarizer_fast_mode,
            generation_priority,
        })
    }

    async fn web_fetch_runtime_execution_context(
        &self,
        generation_priority: noema_providers::GenerationPriority,
    ) -> Result<
        (
            WebFetchBackendHandle,
            WebFetchContext,
            Option<String>,
            Option<String>,
            Option<ProviderAuthFailureTarget>,
        ),
        String,
    > {
        let resolved = super::web_tools::resolve_web_fetch_provider(&self.store)
            .await
            .map_err(|_| "web.fetch provider binding could not be resolved".to_string())?;
        let context = self.web_fetch_runtime_context(generation_priority).await?;
        let target = auth_failure_target(&resolved);
        match self
            .web_backends
            .resolve_fetch(web_backend_request(&resolved))
            .await
        {
            Ok(provider) => {
                let auth_failure_target = auth_failure_target(&resolved);
                Ok((
                    provider,
                    context,
                    resolved.fallback_from,
                    resolved.fallback_reason,
                    auth_failure_target,
                ))
            }
            Err(WebBackendResolverError::Unauthenticated) => {
                if let Some(target) = target {
                    self.mark_provider_account_unauthenticated(&target).await;
                }
                let fallback = super::web_tools::load_default_provider(
                    &self.store,
                    noema_capabilities::CapabilityId::WebFetch,
                )
                .await
                .map_err(|_| "web.fetch fallback account is unavailable".to_string())?;
                let provider = self
                    .web_backends
                    .resolve_fetch(web_backend_request(&fallback))
                    .await
                    .map_err(|_| "web.fetch fallback provider is unavailable".to_string())?;
                Ok((
                    provider,
                    context,
                    Some(resolved.provider_account_id),
                    Some(PROVIDER_ACCOUNT_UNAUTHENTICATED.to_string()),
                    None,
                ))
            }
            Err(WebBackendResolverError::Unavailable) => Err(format!(
                "web.fetch provider '{}' is not available in this daemon",
                resolved.provider_kind
            )),
        }
    }

    async fn web_search_runtime_provider_resolution(
        &self,
    ) -> Result<
        (
            WebSearchBackendHandle,
            Option<String>,
            Option<String>,
            Option<ProviderAuthFailureTarget>,
        ),
        String,
    > {
        let resolved = super::web_tools::resolve_web_search_provider(&self.store)
            .await
            .map_err(|_| "web.search provider binding could not be resolved".to_string())?;
        let target = auth_failure_target(&resolved);
        match self
            .web_backends
            .resolve_search(web_backend_request(&resolved))
            .await
        {
            Ok(provider) => {
                let auth_failure_target = auth_failure_target(&resolved);
                Ok((
                    provider,
                    resolved.fallback_from,
                    resolved.fallback_reason,
                    auth_failure_target,
                ))
            }
            Err(WebBackendResolverError::Unauthenticated) => {
                if let Some(target) = target {
                    self.mark_provider_account_unauthenticated(&target).await;
                }
                let fallback = super::web_tools::load_default_provider(
                    &self.store,
                    noema_capabilities::CapabilityId::WebSearch,
                )
                .await
                .map_err(|_| "web.search fallback account is unavailable".to_string())?;
                let provider = self
                    .web_backends
                    .resolve_search(web_backend_request(&fallback))
                    .await
                    .map_err(|_| "web.search fallback provider is unavailable".to_string())?;
                Ok((
                    provider,
                    Some(resolved.provider_account_id),
                    Some(PROVIDER_ACCOUNT_UNAUTHENTICATED.to_string()),
                    None,
                ))
            }
            Err(WebBackendResolverError::Unavailable) => Err(format!(
                "web.search provider '{}' is not available in this daemon",
                resolved.provider_kind
            )),
        }
    }

    async fn mark_provider_account_unauthenticated(&self, target: &ProviderAuthFailureTarget) {
        let _ = self
            .web_backends
            .record_auth_failure(
                target.provider_account_id.clone(),
                target.credential_revision,
            )
            .await;
    }

    async fn resolve_browser_backend(
        &self,
        resolved: &super::web_tools::ResolvedWebProvider,
    ) -> Result<WebBrowseBackendHandle, WebBrowseError> {
        if resolved.capability_status != noema_providers::ProviderCapabilityStatus::Available {
            return Err(WebBrowseError::Unavailable.with_provider_detail(
                &resolved.provider_kind,
                "resolve_backend",
                format!(
                    "capability status is {}",
                    resolved.capability_status.as_str()
                ),
            ));
        }
        let target = auth_failure_target(resolved);
        match self
            .web_backends
            .resolve_browse(web_backend_request(resolved))
            .await
        {
            Ok(provider) => Ok(provider),
            Err(WebBackendResolverError::Unauthenticated) => {
                if let Some(target) = target {
                    self.mark_provider_account_unauthenticated(&target).await;
                }
                Err(WebBrowseError::Unauthenticated)
            }
            Err(WebBackendResolverError::Unavailable) => Err(WebBrowseError::Unavailable
                .with_provider_detail(
                    &resolved.provider_kind,
                    "resolve_backend",
                    "provider backend is unavailable",
                )),
        }
    }

    async fn execute_web_browse(
        &self,
        owner_key: &str,
        name: &str,
        payload: &Value,
    ) -> Result<Value, WebBrowseError> {
        let mut command =
            parse_command(name, payload).map_err(|error| WebBrowseError::InvalidArguments {
                detail: error.message().to_string(),
            })?;
        let navigation_url = match &command {
            BrowseCommand::Open(request) => Some(request.url.clone()),
            _ => None,
        };
        let gate = self.browser_sessions.gate(owner_key);
        let _guard = gate.lock().await;
        let route = match super::web_tools::resolve_web_browse_route(&self.store).await {
            Ok(route) => route,
            Err(_) => {
                if let Some(obsolete) = self.browser_sessions.remove(owner_key) {
                    let _ = obsolete
                        .backend
                        .execute(&WebBrowseOwner::new(owner_key), BrowseCommand::Close)
                        .await;
                }
                return Err(WebBrowseError::RouteUnavailable);
            }
        };
        let mut session = self.browser_sessions.session(owner_key);
        if session
            .as_ref()
            .is_some_and(|session| session.route.digest != route.digest)
        {
            if let Some(obsolete) = self.browser_sessions.remove(owner_key) {
                let _ = obsolete
                    .backend
                    .execute(&WebBrowseOwner::new(owner_key), BrowseCommand::Close)
                    .await;
            }
            session = None;
        }
        let owner = WebBrowseOwner::new(owner_key);
        let mut state = match session {
            Some(state) => {
                translate_browser_revision(&state, &mut command)?;
                state
            }
            None if matches!(command, BrowseCommand::Open(_)) => {
                let resolved = route
                    .providers
                    .first()
                    .cloned()
                    .ok_or(WebBrowseError::RouteUnavailable)?;
                let backend = self.resolve_browser_backend(&resolved).await?;
                BrowserSessionState {
                    route,
                    active_position: 0,
                    backend,
                    last_navigation_url: None,
                    public_revision: 0,
                    backend_revision: 0,
                    snapshot: None,
                }
            }
            None => return Err(WebBrowseError::SessionNotFound),
        };
        if navigation_url.is_some() {
            state.last_navigation_url = navigation_url;
        }
        let result = state.backend.execute(&owner, command).await;
        if matches!(result, Err(WebBrowseError::Unauthenticated)) {
            let resolved = &state.route.providers[state.active_position];
            if let Some(target) = auth_failure_target(resolved) {
                self.mark_provider_account_unauthenticated(&target).await;
            }
        }
        let mut response = match result {
            Ok(response) => response,
            Err(error) => {
                self.browser_sessions
                    .set_session(owner_key.to_string(), state);
                return Err(error);
            }
        };
        let public_revision = response
            .snapshot
            .as_ref()
            .map(|_| self.browser_sessions.next_revision(owner_key))
            .unwrap_or(state.public_revision);
        update_browser_snapshot_authority(&mut state, &mut response, public_revision);
        self.browser_sessions
            .set_session(owner_key.to_string(), state);
        serde_json::to_value(response).map_err(|_| WebBrowseError::Unavailable)
    }

    pub(super) async fn close_browser_session(
        &self,
        owner_key: &str,
    ) -> Result<Value, WebBrowseError> {
        let gate = self.browser_sessions.gate(owner_key);
        let _guard = gate.lock().await;
        let Some(session) = self.browser_sessions.remove(owner_key) else {
            return serde_json::to_value(BrowseResponse {
                provider: OBSCURA_BROWSER_PROVIDER_ID.to_string(),
                state: "closed".to_string(),
                snapshot: None,
                screenshot: None,
            })
            .map_err(|_| WebBrowseError::Unavailable);
        };
        session
            .backend
            .execute(&WebBrowseOwner::new(owner_key), BrowseCommand::Close)
            .await
            .and_then(|response| {
                serde_json::to_value(response).map_err(|_| WebBrowseError::Unavailable)
            })
    }

    async fn switch_browser_provider(
        &self,
        owner_key: &str,
        payload: &Value,
    ) -> Result<Value, WebBrowseError> {
        let request =
            parse_provider_switch(payload).map_err(|error| WebBrowseError::InvalidArguments {
                detail: error.message().to_string(),
            })?;
        let gate = self.browser_sessions.gate(owner_key);
        let _guard = gate.lock().await;
        let mut source = self
            .browser_sessions
            .session(owner_key)
            .ok_or(WebBrowseError::SessionNotFound)?;
        let route = match super::web_tools::resolve_web_browse_route(&self.store).await {
            Ok(route) => route,
            Err(_) => {
                self.browser_sessions.remove(owner_key);
                let _ = source
                    .backend
                    .execute(&WebBrowseOwner::new(owner_key), BrowseCommand::Close)
                    .await;
                return Err(WebBrowseError::RouteUnavailable);
            }
        };
        if source.route.digest != route.digest {
            self.browser_sessions.remove(owner_key);
            let _ = source
                .backend
                .execute(&WebBrowseOwner::new(owner_key), BrowseCommand::Close)
                .await;
            return Err(WebBrowseError::SessionNotFound);
        }
        validate_browser_switch_revision(&source, request.snapshot_revision)?;
        noema_providers::validate_public_url(&request.url)
            .await
            .map_err(browser_url_policy_error)?;
        let target_position =
            next_browser_route_position(&source).ok_or(WebBrowseError::NoLaterProvider)?;
        source.last_navigation_url = Some(request.url.clone());
        let resolved = &source.route.providers[target_position];
        let target = match self.resolve_browser_backend(resolved).await {
            Ok(target) => target,
            Err(error) => {
                self.browser_sessions
                    .set_session(owner_key.to_string(), source);
                return Err(error);
            }
        };
        let owner = WebBrowseOwner::new(owner_key);
        let result = target
            .execute(
                &owner,
                BrowseCommand::Open(BrowseNavigationRequest {
                    url: request.url,
                    reason: None,
                    wait_until: BrowseWaitUntil::Domcontentloaded,
                }),
            )
            .await;
        let mut response = match result {
            Ok(response) if response.snapshot.is_some() => response,
            Ok(_) => {
                let _ = target.execute(&owner, BrowseCommand::Close).await;
                self.browser_sessions
                    .set_session(owner_key.to_string(), source);
                return Err(WebBrowseError::NavigationFailed);
            }
            Err(error) => {
                if error == WebBrowseError::Unauthenticated
                    && let Some(target) = auth_failure_target(resolved)
                {
                    self.mark_provider_account_unauthenticated(&target).await;
                }
                let _ = target.execute(&owner, BrowseCommand::Close).await;
                self.browser_sessions
                    .set_session(owner_key.to_string(), source);
                return Err(error);
            }
        };
        let _ = source.backend.execute(&owner, BrowseCommand::Close).await;
        source.active_position = target_position;
        source.backend = target;
        let public_revision = self.browser_sessions.next_revision(owner_key);
        update_browser_snapshot_authority(&mut source, &mut response, public_revision);
        self.browser_sessions
            .set_session(owner_key.to_string(), source);
        serde_json::to_value(response).map_err(|_| WebBrowseError::Unavailable)
    }
}

fn publish_captured_task(
    events: &crate::daemon::RuntimeEventRegistry,
    result: &crate::daemon::task_tool::TaskToolResult,
) {
    if result.success
        && let Some(task_id) = result
            .payload
            .get("task")
            .and_then(|task| task.get("task_id"))
            .and_then(Value::as_str)
    {
        events.publish_task(crate::daemon::TaskRuntimeEvent::Changed {
            task_id: task_id.to_string(),
            run_id: None,
        });
        events.publish_work(crate::daemon::WorkRuntimeEvent::Committed {
            workspace_id: "workspace:personal".to_string(),
            task_id: Some(task_id.to_string()),
        });
    }
}

fn translate_browser_revision(
    state: &BrowserSessionState,
    command: &mut BrowseCommand,
) -> Result<(), WebBrowseError> {
    match command {
        BrowseCommand::Interact(request) => {
            let snapshot = state
                .snapshot
                .as_ref()
                .ok_or(WebBrowseError::SessionNotFound)?;
            if request.snapshot_revision != snapshot.revision {
                return Err(WebBrowseError::StaleSnapshot);
            }
            if !snapshot.elements.contains_key(&request.reference) {
                return Err(WebBrowseError::ElementNotFound);
            }
            request.snapshot_revision = state.backend_revision;
        }
        BrowseCommand::History(request) => {
            let snapshot = state
                .snapshot
                .as_ref()
                .ok_or(WebBrowseError::SessionNotFound)?;
            if request.snapshot_revision != snapshot.revision {
                return Err(WebBrowseError::StaleSnapshot);
            }
            request.snapshot_revision = state.backend_revision;
        }
        _ => {}
    }
    Ok(())
}

fn next_browser_route_position(state: &BrowserSessionState) -> Option<usize> {
    let position = state.active_position + 1;
    (position < state.route.providers.len()).then_some(position)
}

fn validate_browser_switch_revision(
    state: &BrowserSessionState,
    revision: Option<u64>,
) -> Result<(), WebBrowseError> {
    match (state.snapshot.as_ref(), revision) {
        (Some(snapshot), Some(revision)) if revision == snapshot.revision => Ok(()),
        (None, None) => Ok(()),
        _ => Err(WebBrowseError::StaleSnapshot),
    }
}

fn is_builtin_browser_tool(name: &str) -> bool {
    noema_capabilities::web::browse::tool_specs()
        .is_ok_and(|specs| specs.iter().any(|spec| spec.name.as_str() == name))
}

fn unadvertised_browser_failure_result(call: &LocalToolCall) -> LocalToolResult {
    let payload = CapabilityError::UnknownOperation.model_payload();
    let sanitizer = UrlPayloadSanitizer;
    LocalToolResult::from_call(call, LocalToolKind::WebBrowse, false, payload.clone(), true)
        .with_persisted(noema_capabilities::PersistedCapabilityPayload {
            arguments: sanitizer.persist_arguments(&call.payload),
            output: sanitizer.persist_output(&payload),
        })
}

fn browser_url_policy_error(error: WebFetchError) -> WebBrowseError {
    match error {
        WebFetchError::UnsupportedScheme | WebFetchError::MalformedUrl => {
            WebBrowseError::InvalidUrl
        }
        WebFetchError::BlockedTarget | WebFetchError::RedirectBlocked => {
            WebBrowseError::BlockedTarget
        }
        WebFetchError::Timeout => WebBrowseError::Timeout,
        _ => WebBrowseError::NavigationFailed,
    }
}

fn update_browser_snapshot_authority(
    state: &mut BrowserSessionState,
    response: &mut BrowseResponse,
    public_revision: u64,
) {
    let Some(snapshot) = response.snapshot.as_mut() else {
        return;
    };
    state.backend_revision = snapshot.snapshot_revision;
    state.public_revision = public_revision;
    snapshot.snapshot_revision = public_revision;
    state.snapshot = Some(BrowserSnapshotContext {
        url: snapshot.url.clone(),
        title: snapshot.title.clone(),
        revision: snapshot.snapshot_revision,
        elements: snapshot
            .elements
            .iter()
            .cloned()
            .map(|element| (element.reference.clone(), element))
            .collect(),
    });
}

pub(super) fn browse_owner_key_for_turn(turn: &SuccessfulProviderTurn) -> String {
    match (&turn.task_id, &turn.task_run_fence) {
        (Some(task_id), Some(fence)) => browse_owner_key_for_task(task_id, fence.task_generation),
        _ => format!("conversation:{}", turn.conversation_id),
    }
}

pub(super) fn browse_owner_key_for_task(task_id: &str, generation: u64) -> String {
    format!("task:{task_id}:{generation}")
}

pub(super) fn browse_owner_key_for_action(
    action: &noema_store::GovernedActionRecord,
) -> Option<String> {
    if let (Some(task_id), Some(generation)) = (
        action.task_id.as_deref(),
        action
            .authorization_context
            .get("task_generation")
            .and_then(Value::as_u64),
    ) {
        return Some(browse_owner_key_for_task(task_id, generation));
    }
    action
        .conversation_id
        .as_deref()
        .map(|conversation_id| format!("conversation:{conversation_id}"))
}

fn browser_validation_failure_result(
    call: &LocalToolCall,
    binding: &noema_capabilities::CapabilityBinding,
    error: WebBrowseError,
) -> LocalToolResult {
    let output = web_actions::browser_failure_output(error, false, None);
    LocalToolResult::from_call(
        call,
        LocalToolKind::WebBrowse,
        false,
        output.payload.clone(),
        true,
    )
    .with_failure(output.failure)
    .with_persisted(binding.persisted_payload(&call.payload, output.persisted_output_source()))
}

fn web_backend_request(resolved: &super::web_tools::ResolvedWebProvider) -> WebBackendRequest {
    WebBackendRequest {
        provider_kind: resolved.provider_kind.clone(),
        provider_account_id: resolved.provider_account_id.clone(),
        auth_method: resolved.auth_method,
        credential_revision: resolved.credential_revision,
    }
}

fn auth_failure_target(
    resolved: &super::web_tools::ResolvedWebProvider,
) -> Option<ProviderAuthFailureTarget> {
    (resolved.auth_method != ProviderAuthMethod::None).then(|| ProviderAuthFailureTarget {
        provider_account_id: resolved.provider_account_id.clone(),
        credential_revision: resolved.credential_revision,
    })
}

struct RuntimeExecutionInvoker<'a> {
    actor: &'a RuntimeActor,
    turn: &'a SuccessfulProviderTurn,
    agent_identity: &'a AgentPromptIdentity,
    call: &'a LocalToolCall,
    result: Mutex<Option<RuntimeExecutionResult>>,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeExecutionResult {
    kind: LocalToolKind,
    requires_provider_continuation: bool,
}

impl<'a> RuntimeExecutionInvoker<'a> {
    fn new(
        actor: &'a RuntimeActor,
        turn: &'a SuccessfulProviderTurn,
        agent_identity: &'a AgentPromptIdentity,
        call: &'a LocalToolCall,
    ) -> Self {
        Self {
            actor,
            turn,
            agent_identity,
            call,
            result: Mutex::new(None),
        }
    }

    fn take_result(&self) -> Option<RuntimeExecutionResult> {
        self.result.lock().expect("runtime result lock").take()
    }
}

impl CapabilityInvoker for RuntimeExecutionInvoker<'_> {
    fn invoke(
        &self,
        invocation: CapabilityInvocation,
    ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>> {
        Box::pin(async move {
            let observed_url_authorization = invocation
                .reviewed_authorization
                .as_ref()
                .is_some_and(|authorization| authorization.action_id == "observed_url");
            if invocation.operation_token.as_str() != invocation.operation.as_str()
                || invocation.operation.as_str() != self.call.name
                || (invocation.arguments != self.call.payload && !observed_url_authorization)
            {
                return Err(CapabilityError::UnknownOperation);
            }
            let mut dispatched_call = self.call.clone();
            dispatched_call.payload = invocation.arguments;
            let result = self
                .actor
                .execute_bound_runtime_tool(self.turn, self.agent_identity, &dispatched_call)
                .await?;
            let output = runtime_capability_output(&result);
            *self.result.lock().expect("runtime result lock") = Some(RuntimeExecutionResult {
                kind: result.kind,
                requires_provider_continuation: result.requires_provider_continuation,
            });
            Ok(output)
        })
    }
}

fn runtime_capability_output(result: &LocalToolResult) -> CapabilityOutput {
    let output = if result.success {
        CapabilityOutput::success(result.payload.clone())
    } else if let Some(failure) = result.failure {
        CapabilityOutput::failed_with_recovery(result.payload.clone(), failure)
    } else {
        CapabilityOutput::failed(result.payload.clone())
    };
    match result.persisted_output_source.clone() {
        Some(source) => output.with_persisted_output_source(source),
        None => output,
    }
}

fn gateway_failure_result(
    call: &LocalToolCall,
    failure: CapabilityDispatchFailure,
) -> LocalToolResult {
    let requires_provider_continuation = failure.error != CapabilityError::OutcomeUncertain;
    let result = LocalToolResult::from_call(
        call,
        LocalToolKind::Gateway,
        false,
        failure.error.model_payload(),
        requires_provider_continuation,
    )
    .with_persisted(failure.persisted);
    if failure.error == CapabilityError::OutcomeUncertain {
        result.with_blocked_outcome_uncertain()
    } else {
        result
    }
}

pub(super) use super::local_tool_results::{
    LocalToolKind, LocalToolResult, agent_identity_after_local_tools,
    local_tool_artifact_reference_item, local_tool_result_action_item,
};

#[cfg(test)]
#[path = "local_tools/tests.rs"]
mod tests;
