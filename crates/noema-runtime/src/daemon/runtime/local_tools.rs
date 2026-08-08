use crate::{
    agent_execution::{ExecutionRole, ToolPolicy},
    search::types::{DUCKDUCKGO_PUBLIC_PROVIDER_ID, SearchRuntimeProvider},
};
use noema_capabilities::{
    CapabilityDispatchFailure, CapabilityError, CapabilityFuture, CapabilityInvocation,
    CapabilityInvoker, CapabilityOutput, CapabilityRegistryRouter, InvokerKey,
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
        capability_failure_code,
    },
    actor::{BrowserSnapshotContext, RuntimeActor},
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
    task_artifact_tool::{
        TaskArtifactReadContext, execute_task_read_artifact, is_task_read_artifact_tool,
    },
    task_tool::{
        TASK_LIST_TOOL, TaskDelegateRuntimeContext, execute_primary_task_tool,
        execute_scoped_task_list_tool, is_primary_task_tool, is_task_report_blocked_tool,
        is_task_submit_plan_tool, is_task_submit_result_tool, is_task_submit_review_tool,
    },
};
use crate::search::tool::{WebSearchToolResult, execute_web_search, is_web_search_tool};
use crate::web_fetch::{
    tool::{WebFetchToolResult, execute_web_fetch, is_web_fetch_tool},
    types::{FetchRuntimeContext, WebFetchRuntimeProvider},
};
use crate::{WebBackendRequest, WebBackendResolverError};
use noema_capabilities::web::browse::parse_command;
use noema_capabilities::web::fetch::WEB_FETCH_TOOL;
use noema_providers::{WebBrowseBackendHandle, WebBrowseOwner};

const PROVIDER_ACCOUNT_UNAUTHENTICATED: &str = "provider account unauthenticated";

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
use web_actions::{insert_web_tool_fallback_metadata, is_provider_account_unauthenticated_payload};

#[path = "native_memory_tools.rs"]
mod native_memory_tools;
use native_memory_tools::execute_native_memory_tool;

struct ProviderAuthFailureTarget {
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
                if let Some((Some(action), _, _)) = &reviewed {
                    let outcome = if dispatch.output.success {
                        GovernedExecutionOutcome::Succeeded
                    } else {
                        GovernedExecutionOutcome::Failed
                    };
                    if self
                        .store
                        .finish_governed_action_execution(
                            &action.action_id,
                            action.revision,
                            outcome,
                            dispatch.persisted.output.as_ref(),
                            (!dispatch.output.success).then_some("tool_declared_failure"),
                        )
                        .await
                        .is_err()
                    {
                        return action_store_failure_result(call);
                    }
                }
                let runtime_result = runtime_invoker.take_result();
                LocalToolResult::from_call(
                    call,
                    runtime_result.map_or(LocalToolKind::Gateway, |result| result.kind),
                    dispatch.output.success,
                    dispatch.output.payload,
                    runtime_result.is_none_or(|result| result.requires_provider_continuation),
                )
                .with_persisted(dispatch.persisted)
            }
            Err(failure) => {
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
        } else if is_task_read_artifact_tool(&call.name) {
            let result = match (&turn.task_id, &turn.task_run_id) {
                (Some(task_id), Some(run_id)) => {
                    execute_task_read_artifact(
                        &self.store,
                        &self.artifact_operations,
                        &TaskArtifactReadContext {
                            task_id: task_id.clone(),
                            run_id: run_id.clone(),
                        },
                        &call.payload,
                    )
                    .await
                }
                _ => Err("task artifact context is unavailable".to_string()),
            };
            let (success, payload) = match result {
                Ok(payload) => (true, payload),
                Err(error) => (false, json!({"error": error})),
            };
            LocalToolResult::from_call(call, LocalToolKind::Gateway, success, payload, true)
        } else if call.name == TASK_LIST_TOOL && turn.task_run_id.is_some() {
            let result = match turn.task_id.as_deref() {
                Some(task_id) => {
                    execute_scoped_task_list_tool(
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
            if result.success
                && let Some(task_id) = result
                    .payload
                    .get("task")
                    .and_then(|task| task.get("task_id"))
                    .and_then(Value::as_str)
            {
                self.runtime_events
                    .publish_task(crate::daemon::TaskRuntimeEvent::Changed {
                        task_id: task_id.to_string(),
                        run_id: None,
                    });
                self.runtime_events
                    .publish_work(crate::daemon::WorkRuntimeEvent::Committed {
                        workspace_id: "workspace:personal".to_string(),
                        task_id: Some(task_id.to_string()),
                    });
            }
            LocalToolResult::from_call(
                call,
                LocalToolKind::Gateway,
                result.success,
                result.payload,
                true,
            )
        } else if is_task_submit_plan_tool(&call.name)
            || is_task_submit_result_tool(&call.name)
            || is_task_submit_review_tool(&call.name)
            || is_task_report_blocked_tool(&call.name)
        {
            match turn.task_terminal_contract.as_ref().map(|contract| {
                contract.validate(
                    turn.initial_model_tools.tool_policy.role(),
                    &call.name,
                    &call.payload,
                )
            }) {
                Some(Err(message)) => LocalToolResult::from_call(
                    call,
                    LocalToolKind::Gateway,
                    false,
                    json!({"code": "invalid_terminal_contract", "message": message}),
                    true,
                ),
                Some(Ok(())) | None => LocalToolResult::from_call(
                    call,
                    LocalToolKind::Gateway,
                    true,
                    call.payload.clone(),
                    false,
                ),
            }
        } else if is_web_search_tool(&call.name) {
            let result = match self.web_search_runtime_provider_resolution().await {
                Ok((provider, fallback_from, fallback_reason, auth_failure_target)) => {
                    let mut result =
                        execute_web_search(&provider, call.call_id.clone(), &call.payload).await;
                    if let Some(target) = auth_failure_target
                        && is_provider_account_unauthenticated_payload(&result.payload)
                    {
                        self.mark_provider_account_unauthenticated(&target).await;
                    }
                    insert_web_tool_fallback_metadata(
                        &mut result.payload,
                        fallback_from.as_deref(),
                        fallback_reason.as_deref(),
                    );
                    if result.success {
                        let source = call.call_id.as_deref().unwrap_or(&turn.turn_id);
                        self.record_search_result_urls(source, &result.payload)
                            .await;
                    }
                    result
                }
                Err(message) => WebSearchToolResult {
                    call_id: call.call_id.clone(),
                    name: noema_capabilities::web::search::WEB_SEARCH_TOOL.to_string(),
                    success: false,
                    payload: json!({ "error": message }),
                },
            };
            LocalToolResult::from_call(
                call,
                LocalToolKind::WebSearch,
                result.success,
                result.payload,
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
            let result = match self
                .web_fetch_runtime_execution_context(generation_priority)
                .await
            {
                Ok((provider, context, fallback_from, fallback_reason, auth_failure_target)) => {
                    let mut result =
                        execute_web_fetch(&provider, &context, call.call_id.clone(), &call.payload)
                            .await;
                    if let Some(target) = auth_failure_target
                        && is_provider_account_unauthenticated_payload(&result.payload)
                    {
                        self.mark_provider_account_unauthenticated(&target).await;
                    }
                    insert_web_tool_fallback_metadata(
                        &mut result.payload,
                        fallback_from.as_deref(),
                        fallback_reason.as_deref(),
                    );
                    if result.success {
                        let source = call.call_id.as_deref().unwrap_or(&turn.turn_id);
                        self.record_fetched_link_urls(source, &result.payload).await;
                    }
                    result
                }
                Err(message) => WebFetchToolResult {
                    call_id: call.call_id.clone(),
                    name: WEB_FETCH_TOOL.to_string(),
                    success: false,
                    payload: json!({ "error": message }),
                },
            };
            LocalToolResult::from_call(
                call,
                LocalToolKind::WebFetch,
                result.success,
                result.payload,
                true,
            )
        } else if call.name.starts_with("web.browse.") {
            let owner_key = browse_owner_key_for_turn(turn);
            let result = self
                .execute_web_browse(
                    WebBrowseOwner::new(owner_key.clone()),
                    &call.name,
                    &call.payload,
                )
                .await;
            let (success, payload) = match result {
                Ok(payload) => (true, payload),
                Err(message) => (false, json!({"error": message})),
            };
            if success {
                let source = call.call_id.as_deref().unwrap_or(&turn.turn_id);
                self.record_browser_urls(source, &payload).await;
                self.remember_browser_snapshot(&owner_key, &call.name, &payload);
            }
            LocalToolResult::from_call(call, LocalToolKind::WebBrowse, success, payload, true)
        } else {
            return Err(CapabilityError::UnknownOperation);
        };
        Ok(result)
    }

    async fn web_fetch_runtime_context(
        &self,
        generation_priority: noema_providers::GenerationPriority,
    ) -> Result<FetchRuntimeContext, String> {
        let summarizer_route = Arc::new(self.web_summary_provider.resolve_route().await.map_err(
            |_| "web.fetch summarizer provider is not available in this daemon".to_string(),
        )?);
        let selection = summarizer_route.selection();
        let summarizer_model = selection.model_profile.clone().ok_or_else(|| {
            "web.fetch summarizer selection has no concrete model profile".to_string()
        })?;
        let summarizer_reasoning_effort = selection.reasoning_effort;
        Ok(FetchRuntimeContext {
            summarizer_route,
            summarizer_model,
            summarizer_reasoning_effort,
            generation_priority,
        })
    }

    async fn web_fetch_runtime_execution_context(
        &self,
        generation_priority: noema_providers::GenerationPriority,
    ) -> Result<
        (
            WebFetchRuntimeProvider,
            FetchRuntimeContext,
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
        let target = ProviderAuthFailureTarget {
            provider_account_id: resolved.provider_account_id.clone(),
            credential_revision: resolved.credential_revision,
        };
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
                self.mark_provider_account_unauthenticated(&target).await;
                let provider = self
                    .web_backends
                    .resolve_fetch(default_fetch_backend_request())
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
            SearchRuntimeProvider,
            Option<String>,
            Option<String>,
            Option<ProviderAuthFailureTarget>,
        ),
        String,
    > {
        let resolved = super::web_tools::resolve_web_search_provider(&self.store)
            .await
            .map_err(|_| "web.search provider binding could not be resolved".to_string())?;
        let target = ProviderAuthFailureTarget {
            provider_account_id: resolved.provider_account_id.clone(),
            credential_revision: resolved.credential_revision,
        };
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
                self.mark_provider_account_unauthenticated(&target).await;
                let provider = self
                    .web_backends
                    .resolve_search(default_search_backend_request())
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

    pub(super) async fn web_browse_runtime_provider_resolution(
        &self,
    ) -> Result<WebBrowseBackendHandle, String> {
        let resolved = super::web_tools::resolve_web_browse_provider(&self.store)
            .await
            .map_err(|_| "web.browse provider binding could not be resolved".to_string())?;
        self.web_backends
            .resolve_browse(web_backend_request(&resolved))
            .await
            .map_err(|_| {
                format!(
                    "web.browse provider '{}' is unavailable",
                    resolved.provider_kind
                )
            })
    }

    async fn execute_web_browse(
        &self,
        owner: WebBrowseOwner,
        name: &str,
        payload: &Value,
    ) -> Result<Value, String> {
        let command = parse_command(name, payload).map_err(|error| error.message().to_string())?;
        let provider = self.web_browse_runtime_provider_resolution().await?;
        provider
            .execute(&owner, command)
            .await
            .and_then(|response| {
                serde_json::to_value(response)
                    .map_err(|_| noema_providers::WebBrowseError::Unavailable)
            })
            .map_err(|error| error.to_string())
    }

    pub(super) fn remember_browser_snapshot(&self, owner: &str, tool_name: &str, payload: &Value) {
        let mut contexts = self
            .browser_snapshot_contexts
            .lock()
            .expect("browser snapshot context lock");
        if tool_name == noema_capabilities::web::browse::WEB_BROWSE_CLOSE_TOOL {
            contexts.remove(owner);
            return;
        }
        let Ok(response) = serde_json::from_value::<noema_capabilities::web::browse::BrowseResponse>(
            payload.clone(),
        ) else {
            return;
        };
        let Some(snapshot) = response.snapshot else {
            return;
        };
        contexts.insert(
            owner.to_string(),
            BrowserSnapshotContext {
                url: snapshot.url,
                title: snapshot.title,
                revision: snapshot.snapshot_revision,
                elements: snapshot
                    .elements
                    .into_iter()
                    .map(|element| (element.reference.clone(), element))
                    .collect(),
            },
        );
    }
}

pub(super) fn browse_owner_key_for_turn(turn: &SuccessfulProviderTurn) -> String {
    match (&turn.task_id, &turn.task_run_fence) {
        (Some(task_id), Some(fence)) => format!("task:{task_id}:{}", fence.task_generation),
        _ => format!("turn:{}", turn.turn_id),
    }
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
        return Some(format!("task:{task_id}:{generation}"));
    }
    action
        .turn_id
        .as_deref()
        .map(|turn_id| format!("turn:{turn_id}"))
}

fn web_backend_request(resolved: &super::web_tools::ResolvedWebProvider) -> WebBackendRequest {
    WebBackendRequest {
        provider_kind: resolved.provider_kind.clone(),
        provider_account_id: resolved.provider_account_id.clone(),
        credential_revision: resolved.credential_revision,
    }
}

fn auth_failure_target(
    resolved: &super::web_tools::ResolvedWebProvider,
) -> Option<ProviderAuthFailureTarget> {
    (resolved.credential_revision > 0).then(|| ProviderAuthFailureTarget {
        provider_account_id: resolved.provider_account_id.clone(),
        credential_revision: resolved.credential_revision,
    })
}

fn default_search_backend_request() -> WebBackendRequest {
    WebBackendRequest {
        provider_kind: DUCKDUCKGO_PUBLIC_PROVIDER_ID.to_string(),
        provider_account_id: format!("provider_account:{DUCKDUCKGO_PUBLIC_PROVIDER_ID}:system"),
        credential_revision: 0,
    }
}

fn default_fetch_backend_request() -> WebBackendRequest {
    let provider_kind = crate::web_fetch::types::DIRECT_HTTP_PROVIDER_ID;
    WebBackendRequest {
        provider_kind: provider_kind.to_string(),
        provider_account_id: format!("provider_account:{provider_kind}:system"),
        credential_revision: 0,
    }
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
            let output = if result.success {
                CapabilityOutput::success(result.payload.clone())
            } else {
                CapabilityOutput::failed(result.payload.clone())
            };
            *self.result.lock().expect("runtime result lock") = Some(RuntimeExecutionResult {
                kind: result.kind,
                requires_provider_continuation: result.requires_provider_continuation,
            });
            Ok(output)
        })
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
        json!({"error": failure.error.to_string()}),
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
