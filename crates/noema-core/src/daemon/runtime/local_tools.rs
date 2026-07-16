use crate::{
    agent_execution::{ExecutionRole, ToolPolicy},
    capability::CapabilityGateway,
    search::types::{DUCKDUCKGO_PUBLIC_PROVIDER_ID, SearchRuntimeProvider},
};
use noema_capabilities::{
    CapabilityDispatchFailure, CapabilityError, CapabilityFuture, CapabilityInvocation,
    CapabilityInvoker, CapabilityOutput, CapabilityRegistryRouter, CapabilityRouter, InvokerKey,
};
use noema_providers::{DEFAULT_TOOL_CLASSIFICATION_MODEL, ProviderAccountStatus};
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
    memory::tool::{MemoryToolRuntimeContext, execute_search_memory, is_search_memory_tool},
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

const EXA_API_BASE_URL: &str = "https://api.exa.ai";
const PROVIDER_ACCOUNT_UNAUTHENTICATED: &str = "provider account unauthenticated";

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

    /// Execute one local/MCP tool under an explicit role policy.
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
        let mcp_invoker = Arc::new(CapabilityGateway {
            store: self.store.clone(),
            system_errors: self.system_errors.clone(),
        });
        let router = CapabilityRegistryRouter::new(vec![
            (
                InvokerKey::new("runtime-execution"),
                runtime_invoker.clone() as Arc<dyn CapabilityInvoker + '_>,
            ),
            (
                InvokerKey::new("mcp"),
                mcp_invoker as Arc<dyn CapabilityInvoker + '_>,
            ),
        ])
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
            let context = MemoryToolRuntimeContext {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                call_site_id: format!("output_{}", call.output_index),
                cwd: turn.cwd.clone(),
                user_input: turn.user_input.clone(),
            };
            LocalToolResult::Memory {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                arguments: call.payload.clone(),
                persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
                result: execute_search_memory(
                    &self.store,
                    self.memory_client(),
                    &context,
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
                execute_task_resume(&self.store, &context, call.call_id.clone(), &call.payload)
                    .await
            } else {
                execute_task_cancel(&self.store, &context, call.call_id.clone(), &call.payload)
                    .await
            };
            if result.success
                && (is_resume || is_cancel)
                && let Some(task_id) = result.payload.get("task_id").and_then(Value::as_str)
            {
                self.task_subscriptions
                    .publish_task(crate::graphql::TaskLiveEvent::Changed {
                        task_id: task_id.to_string(),
                    });
                if is_cancel {
                    let _ = crate::daemon::task_delivery::deliver_task_status_event(
                        &self.store,
                        &self.task_subscriptions,
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
            let provider_account_id = self
                .store
                .active_default_provider_accounts()
                .await
                .ok()
                .and_then(|accounts| {
                    accounts
                        .into_iter()
                        .find(|account| account.provider_kind == turn.provider_kind)
                })
                .map(|account| account.provider_account_id)
                .unwrap_or_else(|| format!("provider_account:{}:default", turn.provider_kind));
            let result = execute_task_delegate(
                &self.store,
                &TaskDelegateRuntimeContext {
                    conversation_id: turn.conversation_id.clone(),
                    turn_id: turn.turn_id.clone(),
                    user_item_id: turn.user_item_id.clone(),
                    agent_id: agent_identity.agent_id.clone(),
                    provider_kind: turn.provider_kind.clone(),
                    provider_account_id,
                    model_profile: turn.model.clone(),
                    reasoning_effort: turn.reasoning_effort,
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
                Ok((provider, fallback_from, fallback_reason, auth_failure_account_id)) => {
                    let mut result =
                        execute_web_search(&provider, call.call_id.clone(), &call.payload).await;
                    if let Some(provider_account_id) = auth_failure_account_id
                        && is_provider_account_unauthenticated_payload(&result.payload)
                    {
                        self.mark_provider_account_unauthenticated(&provider_account_id)
                            .await;
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
                Ok((
                    provider,
                    context,
                    fallback_from,
                    fallback_reason,
                    auth_failure_account_id,
                )) => {
                    let mut result =
                        execute_web_fetch(&provider, &context, call.call_id.clone(), &call.payload)
                            .await;
                    if let Some(provider_account_id) = auth_failure_account_id
                        && is_provider_account_unauthenticated_payload(&result.payload)
                    {
                        self.mark_provider_account_unauthenticated(&provider_account_id)
                            .await;
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
        if let Some(preference) = self
            .store
            .get_auxiliary_model_preference(crate::WEB_FETCH_SUMMARIZER_TASK_ID)
            .await
            .map_err(|_| "web.fetch summarizer preference could not be read".to_string())?
        {
            let summarizer_route = Arc::new(
                self.resolve_provider_route(noema_providers::ProviderSelectionSnapshot::explicit(
                    preference.provider_kind.clone(),
                    preference.provider_account_id,
                    preference.model_profile.clone(),
                    preference.reasoning_effort,
                    Some("web_fetch_summarizer_preference".to_string()),
                ))
                .await
                .map_err(|_| {
                    format!(
                        "web.fetch summarizer provider '{}' is not available in this daemon",
                        preference.provider_kind
                    )
                })?,
            );
            return Ok(FetchRuntimeContext {
                summarizer_provider_kind: preference.provider_kind,
                summarizer_route,
                summarizer_model: preference.model_profile,
                summarizer_reasoning_effort: preference.reasoning_effort,
                generation_priority,
            });
        }

        let summarizer_route = Arc::new(
            self.resolve_provider_route(noema_providers::ProviderSelectionSnapshot::explicit(
                self.default_provider_kind.clone(),
                format!("provider_account:{}:default", self.default_provider_kind),
                DEFAULT_TOOL_CLASSIFICATION_MODEL,
                None,
                Some("web_fetch_summarizer_default".to_string()),
            ))
            .await
            .map_err(|_| {
                "web.fetch summarizer provider is not available in this daemon".to_string()
            })?,
        );
        Ok(FetchRuntimeContext {
            summarizer_provider_kind: self.default_provider_kind.clone(),
            summarizer_route,
            summarizer_model: DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string(),
            summarizer_reasoning_effort: None,
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
            Option<String>,
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
            crate::web_fetch::exa::EXA_FETCH_PROVIDER_ID => {
                let provider_account_id = resolved.provider_account_id.clone();
                let api_key = match self
                    .load_provider_secret_api_key(&resolved.provider_kind, &resolved.account_key)
                    .await
                {
                    Ok(api_key) => api_key,
                    Err(()) => {
                        self.mark_provider_account_unauthenticated(&provider_account_id)
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
                    WebFetchRuntimeProvider::Exa {
                        client: crate::web_fetch::exa::ExaFetchClient {
                            base_url: EXA_API_BASE_URL.to_string(),
                            api_key,
                            http: reqwest::Client::new(),
                        },
                    },
                    context,
                    resolved.fallback_from,
                    resolved.fallback_reason,
                    Some(provider_account_id),
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
            Option<String>,
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
            crate::search::exa::EXA_SEARCH_PROVIDER_ID => {
                let provider_account_id = resolved.provider_account_id.clone();
                let api_key = match self
                    .load_provider_secret_api_key(&resolved.provider_kind, &resolved.account_key)
                    .await
                {
                    Ok(api_key) => api_key,
                    Err(()) => {
                        self.mark_provider_account_unauthenticated(&provider_account_id)
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
                    SearchRuntimeProvider::Exa {
                        client: crate::search::exa::ExaSearchClient {
                            base_url: EXA_API_BASE_URL.to_string(),
                            api_key,
                            http: reqwest::Client::new(),
                        },
                    },
                    resolved.fallback_from,
                    resolved.fallback_reason,
                    Some(provider_account_id),
                ))
            }
            provider_kind => Err(format!(
                "web.search provider '{provider_kind}' is not available in this daemon"
            )),
        }
    }

    async fn load_provider_secret_api_key(
        &self,
        provider_kind: &str,
        account_key: &str,
    ) -> Result<String, ()> {
        crate::provider::secret_input::SecretInputStore::new(
            self.store.provider_account_home(provider_kind, account_key),
        )
        .load_api_key()
        .map_err(|_| ())
    }

    async fn mark_provider_account_unauthenticated(&self, provider_account_id: &str) {
        let _ = self
            .store
            .update_provider_account_status(
                provider_account_id,
                ProviderAccountStatus::Unauthenticated,
                Some("auth_failed"),
                Some(PROVIDER_ACCOUNT_UNAUTHENTICATED),
            )
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

    use crate::{
        NewAuxiliaryModelPreference, WEB_FETCH_SUMMARIZER_TASK_ID,
        daemon::{
            agent_onboarding::AgentPromptIdentity,
            runtime::{
                actor::CodexRuntimeActor, model_tools::ModelTools, tool_lifecycle::LocalToolCall,
                turn::SuccessfulProviderTurn,
            },
        },
    };
    use noema_providers::{
        DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateActionItem, GenerateInput, GenerateRequest,
        GenerateResponse, GenerateResponseItem, GenerateResponseStatus, GenerateStreamEvent,
        ProviderError, ProviderOperations, ProviderToolCapabilities,
    };
    use serde_json::{Value, json};

    #[derive(Debug)]
    struct LocalToolTestProvider {
        default_tool_model: Option<String>,
        requests: Arc<Mutex<Vec<GenerateRequest>>>,
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
        let store = crate::store::tests::test_store().await;
        CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            store.system_error_logger(),
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
        let initial_model_tools = test_web_model_tools();
        SuccessfulProviderTurn {
            conversation_id: "conversation:test".to_string(),
            turn_id: "turn:test".to_string(),
            turn_index: 1,
            user_item_id: "item:user:test".to_string(),
            user_input: "test".to_string(),
            task_id: None,
            task_run_id: None,
            cwd: None,
            provider_kind: "codex".to_string(),
            model: Some("gpt-test".to_string()),
            reasoning_effort: None,
            provider_route: crate::test_support::provider_route(
                noema_providers::ProviderSelectionSnapshot::explicit(
                    "codex",
                    "provider_account:codex:test",
                    "gpt-test",
                    None,
                    Some("test".to_string()),
                ),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default"))),
            ),
            initial_stream_id: "stream:test".to_string(),
            response: GenerateResponse {
                responses: Vec::new(),
                tool_calls: Vec::new(),
                reasoning_items: Vec::new(),
                response_status: GenerateResponseStatus::Final,
                provider: "codex".to_string(),
                model: "gpt-test".to_string(),
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
    async fn forged_foreground_call_is_unknown_and_omits_persistence() {
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

    async fn insert_provider_account_without_web_capabilities(
        store: &crate::NoemaStore,
        provider_account_id: &str,
    ) {
        crate::store::tests::insert_provider_account_for_tests(
            store,
            provider_account_id,
            "codex",
            "runtime-test",
            "codex runtime test",
            noema_providers::ProviderAuthMethod::SecretInput,
            false,
            noema_providers::ProviderAccountStatus::Authenticated,
            json!({}),
        )
        .await;
    }

    async fn insert_provider_account(
        store: &crate::NoemaStore,
        provider_account_id: &str,
        provider_kind: &str,
        account_key: &str,
        status: noema_providers::ProviderAccountStatus,
    ) {
        crate::store::tests::insert_provider_account_for_tests(
            store,
            provider_account_id,
            provider_kind,
            account_key,
            &format!("{provider_kind} {account_key}"),
            noema_providers::ProviderAuthMethod::SecretInput,
            false,
            status,
            json!({}),
        )
        .await;
    }

    async fn insert_provider_capability_binding(
        store: &crate::NoemaStore,
        tool_name: &str,
        provider_account_id: &str,
    ) {
        crate::store::tests::insert_provider_capability_binding_for_tests(
            store,
            tool_name,
            tool_name,
            provider_account_id,
        )
        .await;
    }

    #[tokio::test]
    async fn web_fetch_runtime_context_uses_saved_summarizer_preference() {
        let store = crate::store::tests::test_store().await;
        let account = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .upsert_auxiliary_model_preference(NewAuxiliaryModelPreference {
                task_id: WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
                provider_kind: "foundation_local".to_string(),
                provider_account_id: account.provider_account_id,
                model_profile: "custom-fetch-summary".to_string(),
                reasoning_effort: None,
            })
            .await
            .expect("preference");
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
            store.system_error_logger(),
        )
        .await
        .expect("actor");

        let context = actor
            .web_fetch_runtime_context(noema_providers::GenerationPriority::Foreground)
            .await
            .expect("web fetch context");

        assert_eq!(context.summarizer_provider_kind, "foundation_local");
        assert_eq!(context.summarizer_model, "custom-fetch-summary");
        assert_eq!(
            context.generation_priority,
            noema_providers::GenerationPriority::Foreground
        );
    }

    #[tokio::test]
    async fn web_fetch_runtime_context_uses_saved_summarizer_reasoning_effort() {
        let store = crate::store::tests::test_store().await;
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("account");
        store
            .upsert_auxiliary_model_preference(NewAuxiliaryModelPreference {
                task_id: WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
                provider_kind: "codex".to_string(),
                provider_account_id: account.provider_account_id,
                model_profile: "gpt-5.5".to_string(),
                reasoning_effort: Some(noema_providers::ReasoningEffort::Low),
            })
            .await
            .expect("preference");
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            store.system_error_logger(),
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
        let store = crate::store::tests::test_store().await;
        let account = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .upsert_auxiliary_model_preference(NewAuxiliaryModelPreference {
                task_id: WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
                provider_kind: "foundation_local".to_string(),
                provider_account_id: account.provider_account_id,
                model_profile: "default".to_string(),
                reasoning_effort: None,
            })
            .await
            .expect("preference");
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            store.system_error_logger(),
        )
        .await
        .expect("actor");

        let message = actor
            .web_fetch_runtime_context(noema_providers::GenerationPriority::Foreground)
            .await
            .expect_err("missing provider should fail");

        assert_eq!(
            message,
            "web.fetch summarizer provider 'foundation_local' is not available in this daemon"
        );
    }

    #[tokio::test]
    async fn web_fetch_runtime_context_no_preference_summarizes_with_spec_default_model() {
        let store = crate::store::tests::test_store().await;
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
            store.system_error_logger(),
        )
        .await
        .expect("actor");

        let context = actor
            .web_fetch_runtime_context(noema_providers::GenerationPriority::Foreground)
            .await
            .expect("web fetch context");
        let markdown = format!("{}\n", "Long page paragraph.".repeat(600));
        let summary = crate::web_fetch::summarize::summarize_markdown(
            &context,
            "https://example.com/page",
            Some("Example Page"),
            &markdown,
            2_000,
        )
        .await
        .expect("summarization");

        assert_eq!(summary, "summarized page");
        assert_eq!(context.summarizer_model, DEFAULT_TOOL_CLASSIFICATION_MODEL);
        let requests = requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].model.as_deref(),
            Some(DEFAULT_TOOL_CLASSIFICATION_MODEL)
        );
    }

    #[tokio::test]
    async fn bound_exa_web_search_without_secret_falls_back_to_duckduckgo() {
        let store = crate::store::tests::test_store().await;
        insert_provider_account(
            &store,
            "provider_account:exa:acct_research",
            "exa",
            "acct_research",
            noema_providers::ProviderAccountStatus::Authenticated,
        )
        .await;
        insert_provider_capability_binding(
            &store,
            "web.search",
            "provider_account:exa:acct_research",
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
            store.system_error_logger(),
        )
        .await
        .expect("actor");

        let (provider, fallback_from, fallback_reason, auth_failure_account_id) = actor
            .web_search_runtime_provider_resolution()
            .await
            .expect("provider");

        assert!(matches!(
            provider,
            crate::search::types::SearchRuntimeProvider::DuckDuckGoPublic { .. }
        ));
        assert_eq!(
            fallback_from.as_deref(),
            Some("provider_account:exa:acct_research")
        );
        assert_eq!(
            fallback_reason.as_deref(),
            Some("provider account unauthenticated")
        );
        assert!(auth_failure_account_id.is_none());
    }

    #[tokio::test]
    async fn bound_exa_web_fetch_without_secret_falls_back_to_direct_http() {
        let store = crate::store::tests::test_store().await;
        insert_provider_account(
            &store,
            "provider_account:exa:acct_research",
            "exa",
            "acct_research",
            noema_providers::ProviderAccountStatus::Authenticated,
        )
        .await;
        insert_provider_capability_binding(
            &store,
            "web.fetch",
            "provider_account:exa:acct_research",
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
            store.system_error_logger(),
        )
        .await
        .expect("actor");

        let (provider, context, fallback_from, fallback_reason, auth_failure_account_id) = actor
            .web_fetch_runtime_execution_context(noema_providers::GenerationPriority::Foreground)
            .await
            .expect("context");

        assert!(matches!(
            provider,
            crate::web_fetch::types::WebFetchRuntimeProvider::DirectHttp { .. }
        ));
        assert_eq!(
            fallback_from.as_deref(),
            Some("provider_account:exa:acct_research")
        );
        assert_eq!(
            fallback_reason.as_deref(),
            Some("provider account unauthenticated")
        );
        assert!(auth_failure_account_id.is_none());
        assert_eq!(context.summarizer_model, DEFAULT_TOOL_CLASSIFICATION_MODEL);
        assert_eq!(
            context.generation_priority,
            noema_providers::GenerationPriority::Foreground
        );
    }

    #[tokio::test]
    async fn web_search_local_tool_result_payload_preserves_fallback_metadata() {
        let store = crate::store::tests::test_store().await;
        insert_provider_account_without_web_capabilities(&store, "provider_account:codex:test")
            .await;
        insert_provider_capability_binding(&store, "web.search", "provider_account:codex:test")
            .await;
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            store.system_error_logger(),
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

        assert_eq!(payload["fallback_from"], "provider_account:codex:test");
        assert_eq!(
            payload["fallback_reason"],
            "bound provider account does not declare web.search"
        );
        assert_eq!(payload["error"], "query is required");
    }

    #[tokio::test]
    async fn web_fetch_local_tool_result_payload_preserves_fallback_metadata() {
        let store = crate::store::tests::test_store().await;
        insert_provider_account_without_web_capabilities(&store, "provider_account:codex:test")
            .await;
        insert_provider_capability_binding(&store, "web.fetch", "provider_account:codex:test")
            .await;
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as noema_providers::ProviderHandle,
            )]),
            store.clone(),
            store.system_error_logger(),
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

        assert_eq!(payload["fallback_from"], "provider_account:codex:test");
        assert_eq!(
            payload["fallback_reason"],
            "bound provider account does not declare web.fetch"
        );
        assert_eq!(payload["error"], "url is required");
    }
}
