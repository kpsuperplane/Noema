use crate::{
    agent_execution::{ExecutionRole, ToolPolicy},
    search::types::{DUCKDUCKGO_PUBLIC_PROVIDER_ID, SearchRuntimeProvider},
};
use noema_capabilities::{
    CapabilityDispatchFailure, CapabilityError, CapabilityFuture, CapabilityInvocation,
    CapabilityInvoker, CapabilityOutput, CapabilityRegistryRouter, CapabilityRouter, InvokerKey,
};
use noema_store::GovernedExecutionOutcome;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

use super::{
    action_gateway::{
        GovernedActionPreparation, action_store_failure_result, awaiting_approval_result,
        capability_failure_code,
    },
    actor::RuntimeActor,
    tool_lifecycle::LocalToolCall,
    turn::SuccessfulProviderTurn,
};
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
use noema_capabilities::web::fetch::WEB_FETCH_TOOL;

const PROVIDER_ACCOUNT_UNAUTHENTICATED: &str = "provider account unauthenticated";

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
    /// Execute a foreground tool through the exact initial advertised binding
    /// snapshot and its strict role policy.
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
            .prepare_governed_action(turn, agent_identity, call, &binding)
            .await
        {
            Ok(preparation) => preparation,
            Err(_) => return action_store_failure_result(call),
        };
        let governed = match preparation {
            GovernedActionPreparation::NotRequired => None,
            GovernedActionPreparation::AwaitingApproval(action) => {
                return awaiting_approval_result(call, &action);
            }
            GovernedActionPreparation::Admitted {
                action,
                admission,
                arguments,
            } => Some((
                action,
                admission,
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
        let dispatch = match governed.as_ref() {
            Some((_, admission, arguments)) => {
                router
                    .dispatch_governed(
                        snapshot.clone(),
                        call.name.clone(),
                        arguments.clone(),
                        admission.clone(),
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
                if let Some((Some(action), _, _)) = &governed {
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
                if let Some(result) = runtime_invoker.take_result() {
                    result.with_persisted(dispatch.persisted)
                } else {
                    LocalToolResult::from_call(
                        call,
                        LocalToolKind::Gateway,
                        dispatch.output.success,
                        dispatch.output.payload,
                        true,
                    )
                    .with_persisted(dispatch.persisted)
                }
            }
            Err(failure) => {
                if let Some((Some(action), _, _)) = &governed {
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
            LocalToolResult::from_call(
                call,
                LocalToolKind::Gateway,
                true,
                call.payload.clone(),
                false,
            )
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
    result: Mutex<Option<LocalToolResult>>,
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

    fn take_result(&self) -> Option<LocalToolResult> {
        self.result.lock().expect("runtime result lock").take()
    }
}

impl CapabilityInvoker for RuntimeExecutionInvoker<'_> {
    fn invoke(
        &self,
        invocation: CapabilityInvocation,
    ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>> {
        Box::pin(async move {
            let observed_url_admission = invocation
                .governed_admission
                .as_ref()
                .is_some_and(|admission| admission.action_id == "observed_url");
            if invocation.operation_token.as_str() != invocation.operation.as_str()
                || invocation.operation.as_str() != self.call.name
                || (invocation.arguments != self.call.payload && !observed_url_admission)
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
            *self.result.lock().expect("runtime result lock") = Some(result);
            Ok(output)
        })
    }
}

fn gateway_failure_result(
    call: &LocalToolCall,
    failure: CapabilityDispatchFailure,
) -> LocalToolResult {
    LocalToolResult::from_call(
        call,
        LocalToolKind::Gateway,
        false,
        json!({"error": failure.error.to_string()}),
        true,
    )
    .with_persisted(failure.persisted)
}

pub(super) use super::local_tool_results::{
    LocalToolKind, LocalToolResult, agent_identity_after_local_tools,
    local_tool_artifact_reference_item, local_tool_result_action_item,
    local_tool_result_continuation_input, local_tool_task_reference_item,
};

#[cfg(test)]
#[path = "local_tools/tests.rs"]
mod tests;
