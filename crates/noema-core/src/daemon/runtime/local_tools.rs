use crate::{
    agent_execution::{ExecutionRole, ToolPolicy},
    search::types::{DUCKDUCKGO_PUBLIC_PROVIDER_ID, SearchRuntimeProvider},
};
use noema_capabilities::{
    CapabilityDispatchFailure, CapabilityError, CapabilityFuture, CapabilityInvocation,
    CapabilityInvoker, CapabilityOutput, CapabilityRegistryRouter, CapabilityRouter, InvokerKey,
};
use noema_memory::{MemorySearchAuthority, execute_search_memory, is_search_memory_tool};
use noema_providers::{
    EXA_FETCH_PROVIDER_ID, EXA_SEARCH_PROVIDER_ID, ExaFetchClient, ExaSearchClient,
    ProviderCredential,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

use super::{
    actor::CodexRuntimeActor, tool_lifecycle::LocalToolCall, turn::SuccessfulProviderTurn,
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
    memory::context::project_scope_from_cwd,
    task_artifact_tool::{
        TaskArtifactReadContext, execute_task_read_artifact, is_task_read_artifact_tool,
    },
    task_tool::{
        TaskAccessRuntimeContext, TaskDelegateRuntimeContext, execute_task_cancel,
        execute_task_delegate, execute_task_inspect, execute_task_resume, is_task_cancel_tool,
        is_task_delegate_tool, is_task_inspect_tool, is_task_report_blocked_tool,
        is_task_resume_tool, is_task_submit_result_tool, is_task_submit_review_tool,
    },
};
use crate::search::tool::{WebSearchToolResult, execute_web_search, is_web_search_tool};
use crate::web_fetch::{
    tool::{WebFetchToolResult, execute_web_fetch, is_web_fetch_tool},
    types::{FetchRuntimeContext, WebFetchRuntimeProvider},
};
use noema_capabilities::web::fetch::WEB_FETCH_TOOL;

const PROVIDER_ACCOUNT_UNAUTHENTICATED: &str = "provider account unauthenticated";

struct ProviderAuthFailureTarget {
    provider_account_id: String,
    credential_revision: u64,
}

impl CodexRuntimeActor {
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
        if snapshot.resolve(&call.name).is_none()
            || !policy.strict_for_dispatch().allows_tool(&call.name)
        {
            let failure = CapabilityDispatchFailure::from_snapshot(
                snapshot,
                &call.name,
                &call.payload,
                CapabilityError::Denied,
            );
            return gateway_failure_result(call, failure);
        }

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
        match router
            .dispatch(snapshot.clone(), call.name.clone(), call.payload.clone())
            .await
        {
            Ok(dispatch) => {
                if let Some(mut result) = runtime_invoker.take_result() {
                    result.set_persisted(dispatch.persisted);
                    result
                } else {
                    LocalToolResult::Gateway {
                        call_id: call.call_id.clone(),
                        provider_call_id: call.provider_call_id.clone(),
                        provider_name: call.provider_name.clone(),
                        name: call.name.clone(),
                        arguments: call.payload.clone(),
                        persisted: dispatch.persisted,
                        result: RuntimeCapabilityResult {
                            success: dispatch.output.success,
                            payload: dispatch.output.payload,
                            requires_provider_continuation: true,
                        },
                    }
                }
            }
            Err(failure) => gateway_failure_result(call, failure),
        }
    }

    async fn execute_bound_runtime_tool(
        &self,
        turn: &SuccessfulProviderTurn,
        agent_identity: &AgentPromptIdentity,
        call: &LocalToolCall,
    ) -> Result<LocalToolResult, CapabilityError> {
        let result = if is_search_memory_tool(&call.name) {
            let authority = MemorySearchAuthority::for_conversation(
                &turn.conversation_id,
                project_scope_from_cwd(turn.cwd.as_deref()),
            );
            LocalToolResult::Memory {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                arguments: call.payload.clone(),
                persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
                result: execute_search_memory(
                    self.memory_operations.as_deref(),
                    &authority,
                    call.call_id.clone(),
                    &call.payload,
                )
                .await,
            }
        } else if is_update_own_name_tool(&call.name) {
            let context = AgentNameToolRuntimeContext {
                agent_id: agent_identity.agent_id.clone(),
            };
            LocalToolResult::AgentName {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                arguments: call.payload.clone(),
                persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
                result: execute_update_own_name(
                    &self.store,
                    &context,
                    call.call_id.clone(),
                    &call.payload,
                )
                .await,
            }
        } else if is_artifact_create_local_file_tool(&call.name) {
            let context = ArtifactToolRuntimeContext {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                user_item_id: turn.user_item_id.clone(),
                created_by_actor_id: agent_identity.agent_id.clone(),
                task_id: turn.task_id.clone(),
                task_run_id: turn.task_run_id.clone(),
            };
            LocalToolResult::Artifact {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                arguments: call.payload.clone(),
                persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
                result: execute_artifact_create_local_file(
                    &self.store,
                    &self.artifact_operations,
                    &context,
                    call.call_id.clone(),
                    &call.payload,
                )
                .await,
            }
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
            LocalToolResult::Gateway {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                name: call.name.clone(),
                arguments: call.payload.clone(),
                persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
                result: match result {
                    Ok(payload) => RuntimeCapabilityResult {
                        success: true,
                        payload,
                        requires_provider_continuation: true,
                    },
                    Err(error) => RuntimeCapabilityResult {
                        success: false,
                        payload: json!({"error": error}),
                        requires_provider_continuation: true,
                    },
                },
            }
        } else if is_task_inspect_tool(&call.name)
            || is_task_resume_tool(&call.name)
            || is_task_cancel_tool(&call.name)
        {
            let is_resume = is_task_resume_tool(&call.name);
            let is_cancel = is_task_cancel_tool(&call.name);
            let context = TaskAccessRuntimeContext {
                owner_human_id: "human:local".to_string(),
                actor_id: agent_identity.agent_id.clone(),
            };
            let result = if is_task_inspect_tool(&call.name) {
                execute_task_inspect(&self.store, &context, call.call_id.clone(), &call.payload)
                    .await
            } else if is_resume {
                execute_task_resume(
                    &self.store,
                    self.provider_registry.as_ref(),
                    &context,
                    call.call_id.clone(),
                    &call.payload,
                )
                .await
            } else {
                execute_task_cancel(&self.store, &context, call.call_id.clone(), &call.payload)
                    .await
            };
            if result.success
                && (is_resume || is_cancel)
                && let Some(task_id) = result.payload.get("task_id").and_then(Value::as_str)
            {
                self.runtime_events
                    .publish_task(crate::daemon::TaskRuntimeEvent::Changed {
                        task_id: task_id.to_string(),
                    });
                if is_cancel {
                    let _ = crate::daemon::task_delivery::deliver_task_status_event(
                        &self.store,
                        &self.runtime_events,
                        task_id,
                    )
                    .await;
                }
            }
            LocalToolResult::Gateway {
                call_id: result.call_id,
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                name: result.name,
                arguments: call.payload.clone(),
                persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
                result: RuntimeCapabilityResult {
                    success: result.success,
                    payload: result.payload,
                    requires_provider_continuation: true,
                },
            }
        } else if is_task_submit_result_tool(&call.name)
            || is_task_submit_review_tool(&call.name)
            || is_task_report_blocked_tool(&call.name)
        {
            LocalToolResult::Gateway {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                name: call.name.clone(),
                arguments: call.payload.clone(),
                persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
                result: RuntimeCapabilityResult {
                    success: true,
                    payload: call.payload.clone(),
                    requires_provider_continuation: false,
                },
            }
        } else if is_task_delegate_tool(&call.name) {
            let provider_selection = turn.provider_route.selection();
            let result = execute_task_delegate(
                &self.store,
                self.provider_registry.as_ref(),
                &TaskDelegateRuntimeContext {
                    conversation_id: turn.conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: turn.user_item_id.clone(),
                    agent_id: agent_identity.agent_id.clone(),
                    provider_kind: provider_selection.provider_kind.clone(),
                    provider_account_id: provider_selection.provider_account_id.clone(),
                    model_profile: provider_selection.model_profile.clone(),
                    reasoning_effort: provider_selection.reasoning_effort,
                },
                call.call_id.clone(),
                &call.payload,
            )
            .await;
            LocalToolResult::Gateway {
                call_id: result.call_id,
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                name: result.name,
                arguments: call.payload.clone(),
                persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
                result: RuntimeCapabilityResult {
                    success: result.success,
                    payload: result.payload,
                    requires_provider_continuation: true,
                },
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
                    result
                }
                Err(message) => WebSearchToolResult {
                    call_id: call.call_id.clone(),
                    name: noema_capabilities::web::search::WEB_SEARCH_TOOL.to_string(),
                    success: false,
                    payload: json!({ "error": message }),
                },
            };
            LocalToolResult::WebSearch {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                arguments: call.payload.clone(),
                persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
                result,
            }
        } else if is_web_fetch_tool(&call.name) {
            let generation_priority = match turn.initial_model_tools.tool_policy.role() {
                ExecutionRole::PrimaryConversation => {
                    noema_providers::GenerationPriority::Foreground
                }
                ExecutionRole::TaskExecutor | ExecutionRole::TaskReviewer => {
                    noema_providers::GenerationPriority::Background
                }
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
                    result
                }
                Err(message) => WebFetchToolResult {
                    call_id: call.call_id.clone(),
                    name: WEB_FETCH_TOOL.to_string(),
                    success: false,
                    payload: json!({ "error": message }),
                },
            };
            LocalToolResult::WebFetch {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                arguments: call.payload.clone(),
                persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
                result,
            }
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
        let resolved = self
            .resolved_web_fetch_provider()
            .await
            .map_err(|_| "web.fetch provider binding could not be resolved".to_string())?;
        let context = self.web_fetch_runtime_context(generation_priority).await?;
        match resolved.provider_kind.as_str() {
            crate::web_fetch::types::DIRECT_HTTP_PROVIDER_ID => Ok((
                self.web_fetch_provider.clone(),
                context,
                resolved.fallback_from,
                resolved.fallback_reason,
                None,
            )),
            EXA_FETCH_PROVIDER_ID => {
                let provider_account_id = resolved.provider_account_id.clone();
                let auth_failure_target = ProviderAuthFailureTarget {
                    provider_account_id: provider_account_id.clone(),
                    credential_revision: resolved.credential_revision,
                };
                let api_key = match self
                    .load_provider_secret_api_key(&provider_account_id)
                    .await
                {
                    Ok(api_key) => api_key,
                    Err(()) => {
                        self.mark_provider_account_unauthenticated(&auth_failure_target)
                            .await;
                        return Ok((
                            self.web_fetch_provider.clone(),
                            context,
                            Some(provider_account_id),
                            Some(PROVIDER_ACCOUNT_UNAUTHENTICATED.to_string()),
                            None,
                        ));
                    }
                };
                Ok((
                    WebFetchRuntimeProvider::new(ExaFetchClient::new(api_key).map_err(|_| {
                        "web.fetch Exa client could not be initialized".to_string()
                    })?),
                    context,
                    resolved.fallback_from,
                    resolved.fallback_reason,
                    Some(auth_failure_target),
                ))
            }
            provider_kind => Err(format!(
                "web.fetch provider '{provider_kind}' is not available in this daemon"
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
        let resolved = self
            .resolved_web_search_provider()
            .await
            .map_err(|_| "web.search provider binding could not be resolved".to_string())?;

        match resolved.provider_kind.as_str() {
            DUCKDUCKGO_PUBLIC_PROVIDER_ID => Ok((
                self.search_provider.clone(),
                resolved.fallback_from,
                resolved.fallback_reason,
                None,
            )),
            EXA_SEARCH_PROVIDER_ID => {
                let provider_account_id = resolved.provider_account_id.clone();
                let auth_failure_target = ProviderAuthFailureTarget {
                    provider_account_id: provider_account_id.clone(),
                    credential_revision: resolved.credential_revision,
                };
                let api_key = match self
                    .load_provider_secret_api_key(&provider_account_id)
                    .await
                {
                    Ok(api_key) => api_key,
                    Err(()) => {
                        self.mark_provider_account_unauthenticated(&auth_failure_target)
                            .await;
                        return Ok((
                            self.search_provider.clone(),
                            Some(provider_account_id),
                            Some(PROVIDER_ACCOUNT_UNAUTHENTICATED.to_string()),
                            None,
                        ));
                    }
                };
                Ok((
                    SearchRuntimeProvider::new(ExaSearchClient::new(api_key).map_err(|_| {
                        "web.search Exa client could not be initialized".to_string()
                    })?),
                    resolved.fallback_from,
                    resolved.fallback_reason,
                    Some(auth_failure_target),
                ))
            }
            provider_kind => Err(format!(
                "web.search provider '{provider_kind}' is not available in this daemon"
            )),
        }
    }

    async fn load_provider_secret_api_key(&self, provider_account_id: &str) -> Result<String, ()> {
        self.provider_accounts
            .credentials()
            .exa_api_key(provider_account_id)
            .await
            .map(ProviderCredential::into_secret)
            .map_err(|_| ())
    }

    async fn mark_provider_account_unauthenticated(&self, target: &ProviderAuthFailureTarget) {
        let _ = self
            .provider_accounts
            .operations()
            .record_auth_failure(&target.provider_account_id, target.credential_revision)
            .await;
    }
}

struct RuntimeExecutionInvoker<'a> {
    actor: &'a CodexRuntimeActor,
    turn: &'a SuccessfulProviderTurn,
    agent_identity: &'a AgentPromptIdentity,
    call: &'a LocalToolCall,
    result: Mutex<Option<LocalToolResult>>,
}

impl<'a> RuntimeExecutionInvoker<'a> {
    fn new(
        actor: &'a CodexRuntimeActor,
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
            if invocation.operation_token.as_str() != invocation.operation.as_str()
                || invocation.operation.as_str() != self.call.name
                || invocation.arguments != self.call.payload
            {
                return Err(CapabilityError::UnknownOperation);
            }
            let result = self
                .actor
                .execute_bound_runtime_tool(self.turn, self.agent_identity, self.call)
                .await?;
            let output = if result.success() {
                CapabilityOutput::success(result.payload().clone())
            } else {
                CapabilityOutput::failed(result.payload().clone())
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
    LocalToolResult::Gateway {
        call_id: call.call_id.clone(),
        provider_call_id: call.provider_call_id.clone(),
        provider_name: call.provider_name.clone(),
        name: call.name.clone(),
        arguments: call.payload.clone(),
        persisted: failure.persisted,
        result: RuntimeCapabilityResult {
            success: false,
            payload: json!({"error": failure.error.to_string()}),
            requires_provider_continuation: true,
        },
    }
}

fn insert_web_tool_fallback_metadata(
    payload: &mut Value,
    fallback_from: Option<&str>,
    fallback_reason: Option<&str>,
) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    if let Some(fallback_from) = fallback_from
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        object.insert(
            "fallback_from".to_string(),
            Value::String(fallback_from.to_string()),
        );
    }
    if let Some(fallback_reason) = fallback_reason
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        object.insert(
            "fallback_reason".to_string(),
            Value::String(fallback_reason.to_string()),
        );
    }
}

fn is_provider_account_unauthenticated_payload(payload: &Value) -> bool {
    payload
        .get("error")
        .and_then(Value::as_str)
        .is_some_and(|message| message == PROVIDER_ACCOUNT_UNAUTHENTICATED)
}

pub(super) use super::local_tool_results::{
    LocalToolResult, RuntimeCapabilityResult, agent_identity_after_local_tools,
    local_tool_artifact_reference_item, local_tool_result_action_item,
    local_tool_result_continuation_input, local_tool_task_reference_item,
};
#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    use noema_store::{NewAuxiliaryModelPreference, WEB_FETCH_SUMMARIZER_TASK_ID};

    use crate::daemon::{
        agent_onboarding::AgentPromptIdentity,
        runtime::{
            actor::CodexRuntimeActor, model_tools::ModelTools, tool_lifecycle::LocalToolCall,
            turn::SuccessfulProviderTurn,
        },
    };
    use noema_capabilities::{
        CapabilityError, CapabilityFuture, CapabilityInvoker, CapabilityOutput,
    };
    use noema_providers::{
        GenerateActionItem, GenerateInput, GenerateRequest, GenerateResponse, GenerateResponseItem,
        GenerateResponseStatus, GenerateStreamEvent, ProviderCapabilityAccountReference,
        ProviderError, ProviderOperations, ProviderToolCapabilities,
    };
    use serde_json::{Value, json};

    async fn upsert_ready_auxiliary_model_preference(
        store: &noema_store::NoemaStore,
        preference: NewAuxiliaryModelPreference,
    ) {
        let ready_selection = crate::test_support::ready_provider_selection(
            noema_providers::ProviderSelectionSnapshot::explicit(
                &preference.provider_kind,
                &preference.provider_account_id,
                &preference.model_profile,
                preference.reasoning_effort,
                Some(format!("auxiliary_model_preference:{}", preference.task_id)),
            ),
        );
        store
            .upsert_auxiliary_model_preference_with_ready_selection(preference, &ready_selection)
            .await
            .expect("ready auxiliary model preference");
    }

    #[derive(Debug)]
    struct LocalToolTestProvider {
        default_tool_model: Option<String>,
        requests: Arc<Mutex<Vec<GenerateRequest>>>,
    }

    struct RecordingCapabilityInvoker {
        invocations: Mutex<Vec<noema_capabilities::CapabilityInvocation>>,
        result: Result<CapabilityOutput, CapabilityError>,
    }

    impl RecordingCapabilityInvoker {
        fn returning(result: Result<CapabilityOutput, CapabilityError>) -> Self {
            Self {
                invocations: Mutex::new(Vec::new()),
                result,
            }
        }
    }

    impl Default for RecordingCapabilityInvoker {
        fn default() -> Self {
            Self::returning(Ok(CapabilityOutput::success(
                json!({"document": "contents"}),
            )))
        }
    }

    impl CapabilityInvoker for RecordingCapabilityInvoker {
        fn invoke(
            &self,
            invocation: noema_capabilities::CapabilityInvocation,
        ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>> {
            self.invocations
                .lock()
                .expect("invocation lock")
                .push(invocation);
            let result = self.result.clone();
            Box::pin(async move { result })
        }
    }

    impl ProviderOperations for LocalToolTestProvider {
        fn default_tool_classification_model(&self) -> Option<String> {
            self.default_tool_model.clone()
        }

        fn tool_capabilities(&self, _model: Option<&str>) -> ProviderToolCapabilities {
            ProviderToolCapabilities::default()
        }

        fn generate_streaming<'a>(
            &'a self,
            request: GenerateRequest,
            _on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
        ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>>
        {
            Box::pin(async move {
                self.requests
                    .lock()
                    .expect("requests")
                    .push(request.clone());
                Ok(GenerateResponse {
                    responses: vec![GenerateResponseItem::Text {
                        phase: None,
                        text: "summarized page".to_string(),
                    }],
                    tool_calls: Vec::new(),
                    reasoning_items: Vec::new(),
                    response_status: GenerateResponseStatus::Final,
                    provider: "test".to_string(),
                    model: request
                        .model
                        .unwrap_or_else(|| "missing-test-model".to_string()),
                    response_id: None,
                    usage: None,
                })
            })
        }
    }

    async fn test_actor() -> CodexRuntimeActor {
        let store = crate::test_support::test_store().await;
        CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            crate::test_support::system_error_logger(),
        )
        .await
        .expect("actor")
    }

    impl LocalToolTestProvider {
        fn new(default_tool_model: Option<&str>) -> Self {
            Self {
                default_tool_model: default_tool_model.map(str::to_string),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_requests(
            default_tool_model: Option<&str>,
            requests: Arc<Mutex<Vec<GenerateRequest>>>,
        ) -> Self {
            Self {
                default_tool_model: default_tool_model.map(str::to_string),
                requests,
            }
        }
    }

    fn test_turn() -> SuccessfulProviderTurn {
        test_turn_with_selection(noema_providers::ProviderSelectionSnapshot::explicit(
            "codex",
            "provider_account:codex:default",
            "gpt-test",
            None,
            Some("test".to_string()),
        ))
    }

    fn test_turn_with_selection(
        selection: noema_providers::ProviderSelectionSnapshot,
    ) -> SuccessfulProviderTurn {
        let initial_model_tools = test_web_model_tools();
        let provider_kind = selection.provider_kind.clone();
        let model = selection.model_profile.clone();
        let reasoning_effort = selection.reasoning_effort;
        SuccessfulProviderTurn {
            conversation_id: "conversation:test".to_string(),
            turn_id: "turn:test".to_string(),
            turn_index: 1,
            user_item_id: "item:user:test".to_string(),
            user_input: "test".to_string(),
            task_id: None,
            task_run_id: None,
            cwd: None,
            provider_kind: provider_kind.clone(),
            model: model.clone(),
            reasoning_effort,
            provider_route: crate::test_support::provider_route(
                selection,
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default"))),
            ),
            initial_stream_id: "stream:test".to_string(),
            response: GenerateResponse {
                responses: Vec::new(),
                tool_calls: Vec::new(),
                reasoning_items: Vec::new(),
                response_status: GenerateResponseStatus::Final,
                provider: provider_kind,
                model: model.unwrap_or_else(|| "provider-default".to_string()),
                response_id: None,
                usage: None,
            },
            agent_identity: AgentPromptIdentity {
                agent_id: "agent:primary".to_string(),
                display_name: None,
            },
            runtime_environment: crate::daemon::runtime::turn::current_runtime_environment(None),
            tool_capabilities: ProviderToolCapabilities::default(),
            initial_model_tools: initial_model_tools.clone(),
            continuation_model_tools: initial_model_tools,
            initial_provider_input: GenerateInput::Text("test".to_string()),
        }
    }

    fn test_web_model_tools() -> ModelTools {
        let mut builder = noema_capabilities::CapabilityCatalogBuilder::new();
        let mut policy = crate::agent_execution::ToolPolicy::default();
        let mut prompt_kinds = std::collections::BTreeMap::new();
        for spec in [
            noema_capabilities::web::search::tool_spec().expect("search spec"),
            noema_capabilities::web::fetch::tool_spec().expect("fetch spec"),
        ] {
            let name = spec.name.as_str().to_string();
            let sanitizer: Arc<dyn noema_capabilities::PayloadSanitizer> =
                if name == noema_capabilities::web::fetch::WEB_FETCH_TOOL {
                    Arc::new(noema_capabilities::WebFetchPayloadSanitizer)
                } else {
                    Arc::new(noema_capabilities::RedactingPayloadSanitizer)
                };
            builder
                .add(noema_capabilities::CapabilityBinding::new(
                    spec,
                    noema_capabilities::CapabilityTarget::new(
                        noema_capabilities::InvokerKey::new("runtime-execution"),
                        noema_capabilities::OperationToken::new(name.clone()),
                    ),
                    noema_capabilities::CapabilityAccess {
                        effect: noema_capabilities::CapabilityEffect::ReadOnly,
                        scope: noema_capabilities::CapabilityScope::Global,
                    },
                    sanitizer,
                ))
                .expect("unique binding");
            policy.allow_tool_name(name.clone());
            prompt_kinds.insert(
                name,
                crate::daemon::runtime::model_tools::ModelToolPromptKind::Web,
            );
        }
        ModelTools {
            transport: noema_providers::ProviderToolTransport::Native,
            bindings: builder.build(),
            prompt_rows: Vec::new(),
            unavailable_rows: Vec::new(),
            prompt_kinds,
            tool_policy: policy,
        }
    }

    const TEST_CAPABILITY_NAME: &str = "extension.docs.read";

    fn test_injected_capability_model_tools(
        sanitizer: Arc<dyn noema_capabilities::PayloadSanitizer>,
    ) -> ModelTools {
        let spec = noema_capabilities::ToolSpec::new(
            TEST_CAPABILITY_NAME,
            "Read a document.",
            json!({"type": "object"}),
        )
        .expect("tool spec");
        let mut builder = noema_capabilities::CapabilityCatalogBuilder::new();
        builder
            .add(noema_capabilities::CapabilityBinding::new(
                spec,
                noema_capabilities::CapabilityTarget::new(
                    noema_capabilities::InvokerKey::new("external:test"),
                    noema_capabilities::OperationToken::new("opaque-child-authority"),
                ),
                noema_capabilities::CapabilityAccess {
                    effect: noema_capabilities::CapabilityEffect::ReadOnly,
                    scope: noema_capabilities::CapabilityScope::Global,
                },
                sanitizer,
            ))
            .expect("unique binding");
        let mut policy = crate::agent_execution::ToolPolicy::default();
        policy.allow_tool_name(TEST_CAPABILITY_NAME);
        ModelTools {
            transport: noema_providers::ProviderToolTransport::Native,
            bindings: builder.build(),
            prompt_rows: Vec::new(),
            unavailable_rows: Vec::new(),
            prompt_kinds: std::collections::BTreeMap::from([(
                TEST_CAPABILITY_NAME.to_string(),
                crate::daemon::runtime::model_tools::ModelToolPromptKind::Capability,
            )]),
            tool_policy: policy,
        }
    }

    fn test_tool_call(name: &str, payload: Value) -> LocalToolCall {
        LocalToolCall {
            output_index: 0,
            call_id: Some("call:test".to_string()),
            provider_call_id: Some("provider_call:test".to_string()),
            provider_name: Some("provider_tool:test".to_string()),
            name: name.to_string(),
            payload,
        }
    }

    #[test]
    fn failed_agent_name_tool_result_still_continues_to_provider() {
        let result = super::LocalToolResult::AgentName {
            call_id: Some("call:name".to_string()),
            provider_call_id: Some("provider_call:name".to_string()),
            provider_name: Some("update_own_name".to_string()),
            arguments: json!({"name": ""}),
            persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
            result: crate::daemon::agent_name_tool::AgentNameToolResult {
                call_id: Some("call:name".to_string()),
                name: "update_own_name".to_string(),
                success: false,
                payload: json!({"error": "name is required"}),
            },
        };

        assert!(result.requires_provider_continuation());
    }

    #[tokio::test]
    async fn injected_capability_invoker_receives_the_opaque_advertised_target() {
        let mut actor = test_actor().await;
        let invoker = Arc::new(RecordingCapabilityInvoker::default());
        actor.capability_invokers =
            Arc::from([noema_capabilities::CapabilityInvokerRegistration::new(
                noema_capabilities::InvokerKey::new("external:test"),
                invoker.clone(),
            )]);
        let mut turn = test_turn();
        turn.initial_model_tools = test_injected_capability_model_tools(Arc::new(
            noema_capabilities::OmitPayloadSanitizer,
        ));
        let call = test_tool_call(TEST_CAPABILITY_NAME, json!({"document_id": "document:1"}));

        let result = actor
            .execute_local_tool(
                &turn,
                &AgentPromptIdentity {
                    agent_id: "agent:primary".to_string(),
                    display_name: None,
                },
                &call,
            )
            .await;

        assert!(result.success());
        assert_eq!(result.payload(), &json!({"document": "contents"}));
        let invocations = invoker.invocations.lock().expect("invocation lock");
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].operation.as_str(), TEST_CAPABILITY_NAME);
        assert_eq!(
            invocations[0].operation_token.as_str(),
            "opaque-child-authority"
        );
        assert_eq!(invocations[0].arguments, call.payload);
    }

    #[tokio::test]
    async fn unadvertised_capability_call_returns_a_sanitized_failure_and_continues() {
        let result = test_actor()
            .await
            .execute_local_tool(
                &test_turn(),
                &AgentPromptIdentity {
                    agent_id: "agent:primary".to_string(),
                    display_name: None,
                },
                &test_tool_call("forged.operation", json!({"secret": "do not persist"})),
            )
            .await;

        assert!(!result.success());
        assert_eq!(
            result.payload()["error"],
            "capability operation is unavailable"
        );
        assert_eq!(result.persisted().arguments, None);
        assert_eq!(result.persisted().output, None);
        assert!(result.requires_provider_continuation());
    }

    #[tokio::test]
    async fn unavailable_capability_remains_unadvertised_and_denied() {
        let mut actor = test_actor().await;
        let invoker = Arc::new(RecordingCapabilityInvoker::default());
        actor.capability_invokers =
            Arc::from([noema_capabilities::CapabilityInvokerRegistration::new(
                noema_capabilities::InvokerKey::new("external:test"),
                invoker.clone(),
            )]);
        let turn = test_turn();
        assert!(
            turn.initial_model_tools
                .provider_tools()
                .iter()
                .all(|tool| tool.name.as_str() != TEST_CAPABILITY_NAME)
        );

        let result = actor
            .execute_local_tool(
                &turn,
                &AgentPromptIdentity {
                    agent_id: "agent:primary".to_string(),
                    display_name: None,
                },
                &test_tool_call(TEST_CAPABILITY_NAME, json!({"secret": "do not persist"})),
            )
            .await;

        assert!(!result.success());
        assert_eq!(
            result.payload(),
            &json!({"error": "capability operation is unavailable"})
        );
        assert_eq!(result.persisted().arguments, None);
        assert_eq!(result.persisted().output, None);
        assert!(result.requires_provider_continuation());
        assert!(
            invoker
                .invocations
                .lock()
                .expect("invocation lock")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn advertised_capability_failure_is_sanitized_and_continues_to_provider() {
        let mut actor = test_actor().await;
        actor.capability_invokers =
            Arc::from([noema_capabilities::CapabilityInvokerRegistration::new(
                noema_capabilities::InvokerKey::new("external:test"),
                Arc::new(RecordingCapabilityInvoker::returning(Err(
                    CapabilityError::Failed,
                ))),
            )]);
        let mut turn = test_turn();
        turn.initial_model_tools = test_injected_capability_model_tools(Arc::new(
            noema_capabilities::RedactingPayloadSanitizer,
        ));

        let result = actor
            .execute_local_tool(
                &turn,
                &AgentPromptIdentity {
                    agent_id: "agent:primary".to_string(),
                    display_name: None,
                },
                &test_tool_call(
                    TEST_CAPABILITY_NAME,
                    json!({"document_id": "document:1", "api_key": "private"}),
                ),
            )
            .await;

        assert!(!result.success());
        assert_eq!(
            result.payload(),
            &json!({"error": "capability invocation failed"})
        );
        assert_eq!(
            result.persisted().arguments,
            Some(json!({"document_id": "document:1", "api_key": "[REDACTED]"}))
        );
        assert_eq!(result.persisted().output, Some(json!({"error": "failed"})));
        assert!(result.requires_provider_continuation());
    }

    #[tokio::test]
    async fn tool_declared_failed_capability_output_is_persisted_as_failed() {
        let mut actor = test_actor().await;
        actor.capability_invokers =
            Arc::from([noema_capabilities::CapabilityInvokerRegistration::new(
                noema_capabilities::InvokerKey::new("external:test"),
                Arc::new(RecordingCapabilityInvoker::returning(Ok(
                    CapabilityOutput::failed(json!({
                        "isError": true,
                        "error": "document rejected",
                        "password": "private"
                    })),
                ))),
            )]);
        let mut turn = test_turn();
        turn.initial_model_tools = test_injected_capability_model_tools(Arc::new(
            noema_capabilities::RedactingPayloadSanitizer,
        ));

        let result = actor
            .execute_local_tool(
                &turn,
                &AgentPromptIdentity {
                    agent_id: "agent:primary".to_string(),
                    display_name: None,
                },
                &test_tool_call(TEST_CAPABILITY_NAME, json!({"document_id": "document:1"})),
            )
            .await;

        assert!(!result.success());
        assert_eq!(result.payload()["isError"], true);
        assert_eq!(
            result.persisted().output,
            Some(json!({
                "isError": true,
                "error": "document rejected",
                "password": "[REDACTED]"
            }))
        );
        assert!(result.requires_provider_continuation());
    }

    #[tokio::test]
    async fn task_delegate_uses_the_initialized_reviewer_route() {
        let store = crate::test_support::test_store().await;
        store.ensure_default_actors().await.expect("actors");
        store
            .ensure_default_provider_account()
            .await
            .expect("default account");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticate default account");
        let provider_account_id = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account")
            .provider_account_id;
        store
            .update_provider_account_status(
                &provider_account_id,
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticate foundation account");
        let provider_registry = crate::test_support::ready_test_provider_registry();
        let pool = store
            .ensure_default_task_model_pool_settings_with_readiness(
                "codex",
                provider_registry.as_ref(),
            )
            .await
            .expect("task model settings")
            .into_iter()
            .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
            .expect("simple task model");
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            crate::test_support::system_error_logger(),
        )
        .await
        .expect("actor");
        let turn = test_turn_with_selection(noema_providers::ProviderSelectionSnapshot::explicit(
            "foundation_local",
            provider_account_id.clone(),
            "default",
            None,
            Some("agent:primary".to_string()),
        ));

        let result = actor
            .execute_bound_runtime_tool(
                &turn,
                &AgentPromptIdentity {
                    agent_id: "agent:primary".to_string(),
                    display_name: None,
                },
                &test_tool_call(
                    crate::daemon::task_tool::TASK_DELEGATE_TOOL,
                    json!({
                        "title": "Preserve the source account",
                        "request": "Verify exact task delegation provenance.",
                        "complexity": "simple",
                        "executor_model_pool_entry_id": pool.pool_entry_id,
                        "validation_criteria": [{
                            "description": "The reviewer retains the source provider account."
                        }]
                    }),
                ),
            )
            .await
            .expect("runtime task delegate");

        assert!(
            result.success(),
            "task delegation failed: {}",
            result.payload()
        );
        let task_id = result.payload()["task_id"].as_str().expect("task id");
        let task = store
            .get_task(task_id)
            .await
            .expect("read task")
            .expect("created task");
        assert_eq!(task.reviewer_model.provider_kind, "codex");
        assert_eq!(
            task.reviewer_model.provider_account_id,
            "provider_account:codex:default"
        );
        assert_eq!(
            task.reviewer_model.model_profile.as_deref(),
            Some("gpt-5.6-luna")
        );
        assert_eq!(task.reviewer_model.reasoning_effort, None);
        assert_eq!(
            task.reviewer_model.selection_source.as_deref(),
            Some("agent:task-reviewer")
        );
    }

    #[tokio::test]
    async fn forged_background_call_is_unknown_and_omits_persistence() {
        let actor = test_actor().await;
        let policy = crate::agent_execution::ToolPolicy::for_role(
            crate::agent_execution::ExecutionRole::TaskReviewer,
        );
        let result = actor
            .execute_local_tool_with_policy(
                &test_turn(),
                &AgentPromptIdentity {
                    agent_id: "agent:reviewer".to_string(),
                    display_name: None,
                },
                &test_tool_call("forged.operation", json!({"secret": "do not persist"})),
                &policy,
            )
            .await;

        assert!(!result.success());
        assert_eq!(
            result.payload()["error"],
            "capability operation is unavailable"
        );
        assert_eq!(result.persisted().arguments, None);
        assert_eq!(result.persisted().output, None);
    }

    #[tokio::test]
    async fn known_background_call_denied_by_role_policy_uses_binding_persistence() {
        let actor = test_actor().await;
        let policy = crate::agent_execution::ToolPolicy::for_role(
            crate::agent_execution::ExecutionRole::TaskReviewer,
        );
        let result = actor
            .execute_local_tool_with_policy(
                &test_turn(),
                &AgentPromptIdentity {
                    agent_id: "agent:reviewer".to_string(),
                    display_name: None,
                },
                &test_tool_call(
                    noema_capabilities::web::search::WEB_SEARCH_TOOL,
                    json!({"query": "safe", "api_key": "do not persist"}),
                ),
                &policy,
            )
            .await;

        assert!(!result.success());
        assert_eq!(
            result.payload()["error"],
            "capability invocation was denied"
        );
        assert_eq!(
            result.persisted().arguments,
            Some(json!({"query": "safe", "api_key": "[REDACTED]"}))
        );
        assert_eq!(result.persisted().output, Some(json!({"error": "denied"})));
    }

    async fn ensure_provider_account_without_web_capabilities(
        store: &noema_store::NoemaStore,
    ) -> String {
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("default Codex provider account");
        account.provider_account_id
    }

    async fn create_exa_provider_account(
        store: &noema_store::NoemaStore,
        status: noema_providers::ProviderAccountStatus,
    ) -> String {
        crate::test_support::create_exa_provider_account_for_tests(
            store,
            "Exa runtime test",
            status,
            json!({}),
        )
        .await
        .provider_account_id
    }

    async fn insert_provider_capability_binding(
        store: &noema_store::NoemaStore,
        tool_name: &str,
        provider_account_id: impl Into<String>,
    ) {
        crate::test_support::save_provider_capability_assignment_for_tests(
            store,
            tool_name,
            tool_name,
            ProviderCapabilityAccountReference::persisted(provider_account_id),
        )
        .await;
    }

    #[tokio::test]
    async fn web_fetch_runtime_context_uses_saved_summarizer_preference() {
        let store = crate::test_support::test_store().await;
        let account = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .update_provider_account_status(
                &account.provider_account_id,
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticate foundation");
        upsert_ready_auxiliary_model_preference(
            &store,
            NewAuxiliaryModelPreference {
                task_id: WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
                provider_kind: "foundation_local".to_string(),
                provider_account_id: account.provider_account_id,
                model_profile: "default".to_string(),
                reasoning_effort: None,
            },
        )
        .await;
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([
                (
                    "codex".to_string(),
                    Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                        as noema_providers::ProviderHandle,
                ),
                (
                    "foundation_local".to_string(),
                    Arc::new(LocalToolTestProvider::new(Some("foundation-tool-default")))
                        as noema_providers::ProviderHandle,
                ),
            ]),
            store.clone(),
            crate::test_support::system_error_logger(),
        )
        .await
        .expect("actor");

        let context = actor
            .web_fetch_runtime_context(noema_providers::GenerationPriority::Foreground)
            .await
            .expect("web fetch context");

        assert_eq!(
            context.summarizer_route.selection().provider_kind,
            "foundation_local",
        );
        assert_eq!(context.summarizer_model, "default");
        assert_eq!(
            context.generation_priority,
            noema_providers::GenerationPriority::Foreground
        );
    }

    #[tokio::test]
    async fn web_fetch_runtime_context_uses_saved_summarizer_reasoning_effort() {
        let store = crate::test_support::test_store().await;
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("account");
        store
            .update_provider_account_status(
                &account.provider_account_id,
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticate codex");
        upsert_ready_auxiliary_model_preference(
            &store,
            NewAuxiliaryModelPreference {
                task_id: WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
                provider_kind: "codex".to_string(),
                provider_account_id: account.provider_account_id,
                model_profile: "gpt-5.5".to_string(),
                reasoning_effort: Some(noema_providers::ReasoningEffort::Low),
            },
        )
        .await;
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            crate::test_support::system_error_logger(),
        )
        .await
        .expect("actor");

        let context = actor
            .web_fetch_runtime_context(noema_providers::GenerationPriority::Background)
            .await
            .expect("web fetch context");

        assert_eq!(
            context.summarizer_reasoning_effort,
            Some(noema_providers::ReasoningEffort::Low)
        );
        assert_eq!(
            context.generation_priority,
            noema_providers::GenerationPriority::Background
        );
    }

    #[tokio::test]
    async fn web_fetch_runtime_context_rejects_saved_provider_missing_from_runtime() {
        let store = crate::test_support::test_store().await;
        let account = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .update_provider_account_status(
                &account.provider_account_id,
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticate foundation");
        upsert_ready_auxiliary_model_preference(
            &store,
            NewAuxiliaryModelPreference {
                task_id: WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
                provider_kind: "foundation_local".to_string(),
                provider_account_id: account.provider_account_id,
                model_profile: "default".to_string(),
                reasoning_effort: None,
            },
        )
        .await;
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            crate::test_support::system_error_logger(),
        )
        .await
        .expect("actor");

        let message = actor
            .web_fetch_runtime_context(noema_providers::GenerationPriority::Foreground)
            .await
            .expect_err("missing provider should fail");

        assert_eq!(
            message,
            "web.fetch summarizer provider is not available in this daemon"
        );
    }

    #[tokio::test]
    async fn web_fetch_runtime_context_uses_initialized_auxiliary_selection() {
        let store = crate::test_support::test_store().await;
        let requests = Arc::new(Mutex::new(Vec::new()));
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::with_requests(
                    Some("configured-tool-override"),
                    Arc::clone(&requests),
                )) as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            crate::test_support::system_error_logger(),
        )
        .await
        .expect("actor");

        let context = actor
            .web_fetch_runtime_context(noema_providers::GenerationPriority::Foreground)
            .await
            .expect("web fetch context");
        let markdown = format!("{}\n", "Long page paragraph.".repeat(600));
        let summary = noema_providers::summarize_markdown(
            &context,
            "https://example.com/page",
            Some("Example Page"),
            &markdown,
            2_000,
        )
        .await
        .expect("summarization");

        assert_eq!(summary, "summarized page");
        assert_eq!(context.summarizer_model, "gpt-5.6-luna");
        let requests = requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].model.as_deref(), Some("gpt-5.6-luna"));
    }

    #[tokio::test]
    async fn bound_exa_web_search_without_secret_falls_back_to_duckduckgo() {
        let store = crate::test_support::test_store().await;
        let provider_account_id = create_exa_provider_account(
            &store,
            noema_providers::ProviderAccountStatus::Authenticated,
        )
        .await;
        insert_provider_capability_binding(&store, "web.search", provider_account_id.clone()).await;
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            crate::test_support::system_error_logger(),
        )
        .await
        .expect("actor");

        let (provider, fallback_from, fallback_reason, auth_failure_account_id) = actor
            .web_search_runtime_provider_resolution()
            .await
            .expect("provider");

        assert_eq!(
            provider.backend_id(),
            crate::search::types::DUCKDUCKGO_PUBLIC_PROVIDER_ID,
        );
        assert_eq!(fallback_from.as_deref(), Some(provider_account_id.as_str()));
        assert_eq!(
            fallback_reason.as_deref(),
            Some("provider account unauthenticated")
        );
        assert!(auth_failure_account_id.is_none());
        let account = store
            .get_provider_account(&provider_account_id)
            .await
            .expect("provider account")
            .expect("Exa account");
        assert_eq!(
            account.status,
            noema_providers::ProviderAccountStatus::Unauthenticated
        );
        assert_eq!(account.last_error_code.as_deref(), Some("auth_failed"));
    }

    #[tokio::test]
    async fn bound_exa_web_fetch_without_secret_falls_back_to_direct_http() {
        let store = crate::test_support::test_store().await;
        let provider_account_id = create_exa_provider_account(
            &store,
            noema_providers::ProviderAccountStatus::Authenticated,
        )
        .await;
        insert_provider_capability_binding(&store, "web.fetch", provider_account_id.clone()).await;
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            crate::test_support::system_error_logger(),
        )
        .await
        .expect("actor");

        let (provider, context, fallback_from, fallback_reason, auth_failure_account_id) = actor
            .web_fetch_runtime_execution_context(noema_providers::GenerationPriority::Foreground)
            .await
            .expect("context");

        assert_eq!(
            provider.backend_id(),
            crate::web_fetch::types::DIRECT_HTTP_PROVIDER_ID,
        );
        assert_eq!(fallback_from.as_deref(), Some(provider_account_id.as_str()));
        assert_eq!(
            fallback_reason.as_deref(),
            Some("provider account unauthenticated")
        );
        assert!(auth_failure_account_id.is_none());
        assert_eq!(context.summarizer_model, "gpt-5.6-luna");
        assert_eq!(
            context.generation_priority,
            noema_providers::GenerationPriority::Foreground
        );
    }

    #[tokio::test]
    async fn web_search_local_tool_result_payload_preserves_fallback_metadata() {
        let store = crate::test_support::test_store().await;
        let provider_account_id = ensure_provider_account_without_web_capabilities(&store).await;
        insert_provider_capability_binding(&store, "web.search", provider_account_id.clone()).await;
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            crate::test_support::system_error_logger(),
        )
        .await
        .expect("actor");

        let result = actor
            .execute_local_tool(
                &test_turn(),
                &AgentPromptIdentity {
                    agent_id: "agent:primary".to_string(),
                    display_name: None,
                },
                &test_tool_call(
                    "web.search",
                    json!({
                        "query": "   ",
                    }),
                ),
            )
            .await;

        let GenerateActionItem::ToolResult { payload, .. } =
            super::local_tool_result_action_item(&result)
        else {
            panic!("expected tool result action item");
        };

        assert_eq!(payload["fallback_from"], provider_account_id);
        assert_eq!(
            payload["fallback_reason"],
            "bound provider account does not declare web.search"
        );
        assert_eq!(payload["error"], "query is required");
    }

    #[tokio::test]
    async fn web_fetch_local_tool_result_payload_preserves_fallback_metadata() {
        let store = crate::test_support::test_store().await;
        let provider_account_id = ensure_provider_account_without_web_capabilities(&store).await;
        insert_provider_capability_binding(&store, "web.fetch", provider_account_id.clone()).await;
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            crate::test_support::system_error_logger(),
        )
        .await
        .expect("actor");

        let result = actor
            .execute_local_tool(
                &test_turn(),
                &AgentPromptIdentity {
                    agent_id: "agent:primary".to_string(),
                    display_name: None,
                },
                &test_tool_call(
                    "web.fetch",
                    json!({
                        "url": "",
                    }),
                ),
            )
            .await;

        let GenerateActionItem::ToolResult { payload, .. } =
            super::local_tool_result_action_item(&result)
        else {
            panic!("expected tool result action item");
        };

        assert_eq!(payload["fallback_from"], provider_account_id);
        assert_eq!(
            payload["fallback_reason"],
            "bound provider account does not declare web.fetch"
        );
        assert_eq!(payload["error"], "url is required");
    }
}
