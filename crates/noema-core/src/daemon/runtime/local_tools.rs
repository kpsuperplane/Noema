use crate::{
    ProviderAccountStatus,
    agent_execution::{ExecutionRole, ToolPolicy},
    capability::{CapabilityGateway, GatewayToolProposal, GatewayToolResult},
    provider::{DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateActionItem},
    search::types::{DUCKDUCKGO_PUBLIC_PROVIDER_ID, SearchRuntimeProvider},
};
use serde_json::{Value, json};

use super::{
    actor::CodexRuntimeActor, tool_lifecycle::LocalToolCall, turn::SuccessfulProviderTurn,
};
use crate::daemon::TurnTranscriptItem;
use crate::daemon::{
    agent_name_tool::{
        AgentNameToolResult, AgentNameToolRuntimeContext, execute_update_own_name,
        is_update_own_name_tool,
    },
    agent_onboarding::AgentPromptIdentity,
    artifact_tool::{
        ArtifactToolResult, ArtifactToolRuntimeContext, execute_artifact_create_local_file,
        is_artifact_create_local_file_tool,
    },
    memory::tool::{
        MemoryToolResult, MemoryToolRuntimeContext, execute_search_memory, is_search_memory_tool,
    },
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
    tool::{WEB_FETCH_TOOL, WebFetchToolResult, execute_web_fetch, is_web_fetch_tool},
    types::{FetchRuntimeContext, WebFetchRuntimeProvider},
};

const EXA_API_BASE_URL: &str = "https://api.exa.ai";
const PROVIDER_ACCOUNT_UNAUTHENTICATED: &str = "provider account unauthenticated";

impl CodexRuntimeActor {
    /// Execute a foreground tool with the legacy primary-agent policy.
    ///
    /// Background execution should call [`Self::execute_local_tool_with_policy`]
    /// so the same allowlist used to build provider-visible tools is enforced
    /// again at dispatch.
    pub(super) async fn execute_local_tool(
        &self,
        turn: &SuccessfulProviderTurn,
        agent_identity: &AgentPromptIdentity,
        call: &LocalToolCall,
    ) -> LocalToolResult {
        let policy = ToolPolicy::for_role(ExecutionRole::PrimaryConversation);
        self.execute_local_tool_with_policy(turn, agent_identity, call, &policy)
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
        let gateway = CapabilityGateway {
            store: &self.store,
            system_errors: &self.system_errors,
        };
        if !policy.allows_tool(&call.name) {
            return LocalToolResult::Gateway {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                name: call.name.clone(),
                arguments: call.payload.clone(),
                result: GatewayToolResult {
                    success: false,
                    payload: json!({"error": "tool_not_allowed_for_execution_role"}),
                    requires_provider_continuation: true,
                },
            };
        }
        if is_search_memory_tool(&call.name) {
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
                result: execute_artifact_create_local_file(
                    &self.store,
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
                result: match result {
                    Ok(payload) => GatewayToolResult {
                        success: true,
                        payload,
                        requires_provider_continuation: true,
                    },
                    Err(error) => GatewayToolResult {
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
                result: GatewayToolResult {
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
                result: GatewayToolResult {
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
                result: GatewayToolResult {
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
                    name: crate::search::tool::WEB_SEARCH_TOOL.to_string(),
                    success: false,
                    payload: json!({ "error": message }),
                },
            };
            LocalToolResult::WebSearch {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                arguments: call.payload.clone(),
                result,
            }
        } else if is_web_fetch_tool(&call.name) {
            let result = match self.web_fetch_runtime_execution_context().await {
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
                result,
            }
        } else {
            let proposal = GatewayToolProposal {
                name: &call.name,
                payload: &call.payload,
            };
            LocalToolResult::Gateway {
                call_id: call.call_id.clone(),
                provider_call_id: call.provider_call_id.clone(),
                provider_name: call.provider_name.clone(),
                name: call.name.clone(),
                arguments: call.payload.clone(),
                result: gateway
                    .execute_tool_proposal_with_policy(proposal, policy)
                    .await,
            }
        }
    }

    async fn web_fetch_runtime_context(&self) -> Result<FetchRuntimeContext, String> {
        if let Some(preference) = self
            .store
            .get_auxiliary_model_preference(crate::WEB_FETCH_SUMMARIZER_TASK_ID)
            .await
            .map_err(|_| "web.fetch summarizer preference could not be read".to_string())?
        {
            let summarizer_provider =
                self.provider_for_kind(&preference.provider_kind)
                    .map_err(|_| {
                        format!(
                            "web.fetch summarizer provider '{}' is not available in this daemon",
                            preference.provider_kind
                        )
                    })?;
            return Ok(FetchRuntimeContext {
                summarizer_provider_kind: preference.provider_kind,
                summarizer_provider,
                summarizer_model: preference.model_profile,
                summarizer_reasoning_effort: preference.reasoning_effort,
            });
        }

        let summarizer_provider = self.default_provider().map_err(|_| {
            "web.fetch summarizer provider is not available in this daemon".to_string()
        })?;
        Ok(FetchRuntimeContext {
            summarizer_provider_kind: self.default_provider_kind.clone(),
            summarizer_provider,
            summarizer_model: DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string(),
            summarizer_reasoning_effort: None,
        })
    }

    async fn web_fetch_runtime_execution_context(
        &self,
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
        let context = self.web_fetch_runtime_context().await?;
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

#[derive(Debug, Clone)]
pub(super) enum LocalToolResult {
    Memory {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        result: MemoryToolResult,
    },
    AgentName {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        result: AgentNameToolResult,
    },
    Artifact {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        result: ArtifactToolResult,
    },
    WebSearch {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        result: WebSearchToolResult,
    },
    WebFetch {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        arguments: Value,
        result: WebFetchToolResult,
    },
    Gateway {
        call_id: Option<String>,
        provider_call_id: Option<String>,
        provider_name: Option<String>,
        name: String,
        arguments: Value,
        result: GatewayToolResult,
    },
}

impl LocalToolResult {
    pub(super) fn call_id(&self) -> Option<&String> {
        match self {
            Self::Memory { call_id, .. }
            | Self::AgentName { call_id, .. }
            | Self::Artifact { call_id, .. }
            | Self::WebSearch { call_id, .. }
            | Self::WebFetch { call_id, .. } => call_id.as_ref(),
            Self::Gateway { call_id, .. } => call_id.as_ref(),
        }
    }

    pub(super) fn provider_call_id(&self) -> Option<&String> {
        match self {
            Self::Memory {
                provider_call_id, ..
            }
            | Self::AgentName {
                provider_call_id, ..
            }
            | Self::Artifact {
                provider_call_id, ..
            }
            | Self::WebSearch {
                provider_call_id, ..
            }
            | Self::WebFetch {
                provider_call_id, ..
            }
            | Self::Gateway {
                provider_call_id, ..
            } => provider_call_id.as_ref(),
        }
    }

    pub(super) fn provider_name(&self) -> Option<&String> {
        match self {
            Self::Memory { provider_name, .. }
            | Self::AgentName { provider_name, .. }
            | Self::Artifact { provider_name, .. }
            | Self::WebSearch { provider_name, .. }
            | Self::WebFetch { provider_name, .. }
            | Self::Gateway { provider_name, .. } => provider_name.as_ref(),
        }
    }

    pub(super) fn arguments(&self) -> &Value {
        match self {
            Self::Memory { arguments, .. }
            | Self::AgentName { arguments, .. }
            | Self::Artifact { arguments, .. }
            | Self::WebSearch { arguments, .. }
            | Self::WebFetch { arguments, .. }
            | Self::Gateway { arguments, .. } => arguments,
        }
    }

    pub(super) fn name(&self) -> &str {
        match self {
            Self::Memory { result, .. } => &result.name,
            Self::AgentName { result, .. } => &result.name,
            Self::Artifact { result, .. } => &result.name,
            Self::WebSearch { result, .. } => &result.name,
            Self::WebFetch { result, .. } => &result.name,
            Self::Gateway { name, .. } => name,
        }
    }

    pub(super) fn success(&self) -> bool {
        match self {
            Self::Memory { result, .. } => result.success,
            Self::AgentName { result, .. } => result.success,
            Self::Artifact { result, .. } => result.success,
            Self::WebSearch { result, .. } => result.success,
            Self::WebFetch { result, .. } => result.success,
            Self::Gateway { result, .. } => result.success,
        }
    }

    pub(super) fn payload(&self) -> &Value {
        match self {
            Self::Memory { result, .. } => &result.payload,
            Self::AgentName { result, .. } => &result.payload,
            Self::Artifact { result, .. } => &result.payload,
            Self::WebSearch { result, .. } => &result.payload,
            Self::WebFetch { result, .. } => &result.payload,
            Self::Gateway { result, .. } => &result.payload,
        }
    }

    pub(super) fn transcript_payload(&self) -> Value {
        json!({
            "call_id": self.call_id(),
            "provider_call_id": self.provider_call_id(),
            "provider_name": self.provider_name(),
            "name": self.name(),
            "arguments": self.arguments(),
            "success": self.success(),
            "payload": self.payload(),
        })
    }

    pub(super) fn requires_provider_continuation(&self) -> bool {
        match self {
            Self::Memory { .. }
            | Self::Artifact { .. }
            | Self::WebSearch { .. }
            | Self::WebFetch { .. } => true,
            Self::AgentName { .. } => true,
            Self::Gateway { result, .. } => result.requires_provider_continuation,
        }
    }
}

pub(super) fn agent_identity_after_local_tools(
    current: &AgentPromptIdentity,
    results: &[LocalToolResult],
) -> AgentPromptIdentity {
    let mut agent_identity = current.clone();
    for result in results {
        let LocalToolResult::AgentName { result, .. } = result else {
            continue;
        };
        if result.success {
            agent_identity.display_name = result
                .payload
                .get("display_name")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
    }
    agent_identity
}

pub(super) fn local_tool_result_continuation_input(results: &[&LocalToolResult]) -> Value {
    json!({
        "type": "NOEMA_LOCAL_TOOL_RESULT",
        "results": results
            .iter()
            .map(|result| local_tool_result_payload(result))
            .collect::<Vec<_>>(),
    })
}

fn local_tool_result_payload(result: &LocalToolResult) -> Value {
    json!({
        "call_id": result.call_id(),
        "provider_call_id": result.provider_call_id(),
        "provider_name": result.provider_name(),
        "name": result.name(),
        "success": result.success(),
        "payload": result.payload(),
    })
}

pub(super) fn local_tool_result_action_item(result: &LocalToolResult) -> GenerateActionItem {
    GenerateActionItem::ToolResult {
        call_id: result.call_id().cloned(),
        provider_call_id: result.provider_call_id().cloned(),
        provider_name: result.provider_name().cloned(),
        name: Some(result.name().to_string()),
        success: Some(result.success()),
        payload: result.payload().clone(),
    }
}

pub(super) fn local_tool_artifact_reference_item(
    result: &LocalToolResult,
) -> Option<TurnTranscriptItem> {
    if !matches!(result, LocalToolResult::Artifact { .. }) || !result.success() {
        return None;
    }
    let payload = result.payload();
    let artifact_id = payload.get("artifact_id")?.as_str()?.to_string();
    let title = payload.get("title")?.as_str()?.to_string();
    let artifact_kind = payload.get("artifact_kind")?.as_str()?.to_string();
    let storage_kind = payload.get("storage_kind")?.as_str()?.to_string();
    let artifact_version_id = payload
        .get("current_version_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let download_url = payload
        .get("download_url")
        .and_then(Value::as_str)
        .map(str::to_string);
    let media_type = payload
        .get("media_type")
        .and_then(Value::as_str)
        .map(str::to_string);

    Some(TurnTranscriptItem::ArtifactReference {
        artifact_id,
        artifact_version_id,
        title,
        artifact_kind,
        storage_kind,
        external_url: None,
        download_url,
        media_type,
    })
}

/// Build a durable task marker after successful primary task creation or
/// control so agent-issued resume/cancel actions update the source transcript
/// through the normal conversation sink.
pub(super) fn local_tool_task_reference_item(
    result: &LocalToolResult,
) -> Option<TurnTranscriptItem> {
    if !matches!(
        result.name(),
        crate::daemon::task_tool::TASK_DELEGATE_TOOL
            | crate::daemon::task_tool::TASK_RESUME_TOOL
            | crate::daemon::task_tool::TASK_CANCEL_TOOL
    ) || !result.success()
    {
        return None;
    }
    let payload = result.payload();
    let task_id = payload.get("task_id")?.as_str()?.to_string();
    let title = payload.get("title")?.as_str()?.to_string();
    let status = payload
        .get("status")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<crate::TaskStatus>().ok())?;
    let revision = payload
        .get("revision")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    Some(TurnTranscriptItem::TaskReference {
        task_id,
        title,
        status: status.as_str().to_string(),
        revision,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    use crate::{
        NewAuxiliaryModelPreference, ProviderError, WEB_FETCH_SUMMARIZER_TASK_ID,
        daemon::{
            agent_onboarding::AgentPromptIdentity,
            runtime::{
                actor::CodexRuntimeActor, handle::RuntimeModelProvider, model_tools::ModelTools,
                tool_lifecycle::LocalToolCall, turn::SuccessfulProviderTurn,
            },
        },
        provider::{
            DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateActionItem, GenerateInput, GenerateRequest,
            GenerateResponse, GenerateResponseItem, GenerateResponseStatus, GenerateStreamEvent,
            ProviderToolCapabilities,
        },
    };
    use serde_json::{Value, json};

    #[derive(Debug)]
    struct LocalToolTestProvider {
        default_tool_model: Option<String>,
        requests: Arc<Mutex<Vec<GenerateRequest>>>,
    }

    impl RuntimeModelProvider for LocalToolTestProvider {
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
            tool_capabilities: ProviderToolCapabilities::default(),
            continuation_model_tools: ModelTools {
                native: Vec::new(),
                legacy_builtin_envelope_tools: Vec::new(),
                prompt_rows: Vec::new(),
                unavailable_rows: Vec::new(),
                tool_policy: crate::agent_execution::ToolPolicy::default(),
            },
            rendered_tools: String::new(),
            rendered_continuation_tools: String::new(),
            initial_provider_input: GenerateInput::Text("test".to_string()),
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
            result: super::AgentNameToolResult {
                call_id: Some("call:name".to_string()),
                name: "update_own_name".to_string(),
                success: false,
                payload: json!({"error": "name is required"}),
            },
        };

        assert!(result.requires_provider_continuation());
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
            crate::ProviderAuthMethod::SecretInput,
            false,
            crate::ProviderAccountStatus::Authenticated,
            json!({}),
        )
        .await;
    }

    async fn insert_provider_account(
        store: &crate::NoemaStore,
        provider_account_id: &str,
        provider_kind: &str,
        account_key: &str,
        status: crate::ProviderAccountStatus,
    ) {
        crate::store::tests::insert_provider_account_for_tests(
            store,
            provider_account_id,
            provider_kind,
            account_key,
            &format!("{provider_kind} {account_key}"),
            crate::ProviderAuthMethod::SecretInput,
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
                        as Arc<dyn RuntimeModelProvider>,
                ),
                (
                    "foundation_local".to_string(),
                    Arc::new(LocalToolTestProvider::new(Some("foundation-tool-default")))
                        as Arc<dyn RuntimeModelProvider>,
                ),
            ]),
            store.clone(),
            store.system_error_logger(),
        )
        .await
        .expect("actor");

        let context = actor
            .web_fetch_runtime_context()
            .await
            .expect("web fetch context");

        assert_eq!(context.summarizer_provider_kind, "foundation_local");
        assert_eq!(context.summarizer_model, "custom-fetch-summary");
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
                reasoning_effort: Some(crate::provider::ReasoningEffort::Low),
            })
            .await
            .expect("preference");
        let actor = CodexRuntimeActor::new(
            "codex".to_string(),
            HashMap::from([(
                "codex".to_string(),
                Arc::new(LocalToolTestProvider::new(Some("codex-tool-default")))
                    as Arc<dyn RuntimeModelProvider>,
            )]),
            store.clone(),
            store.system_error_logger(),
        )
        .await
        .expect("actor");

        let context = actor
            .web_fetch_runtime_context()
            .await
            .expect("web fetch context");

        assert_eq!(
            context.summarizer_reasoning_effort,
            Some(crate::provider::ReasoningEffort::Low)
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
                    as Arc<dyn RuntimeModelProvider>,
            )]),
            store.clone(),
            store.system_error_logger(),
        )
        .await
        .expect("actor");

        let message = actor
            .web_fetch_runtime_context()
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
                )) as Arc<dyn RuntimeModelProvider>,
            )]),
            store.clone(),
            store.system_error_logger(),
        )
        .await
        .expect("actor");

        let context = actor
            .web_fetch_runtime_context()
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
            crate::ProviderAccountStatus::Authenticated,
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
                    as Arc<dyn RuntimeModelProvider>,
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
            crate::ProviderAccountStatus::Authenticated,
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
                    as Arc<dyn RuntimeModelProvider>,
            )]),
            store.clone(),
            store.system_error_logger(),
        )
        .await
        .expect("actor");

        let (provider, context, fallback_from, fallback_reason, auth_failure_account_id) = actor
            .web_fetch_runtime_execution_context()
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
                    as Arc<dyn RuntimeModelProvider>,
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
                    as Arc<dyn RuntimeModelProvider>,
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
