//! Provider/tool continuation loop for supervised task runs.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use noema_store::{
    RuntimeDebugMetadata, RuntimeDebugScope, RuntimeDebugSpanCategory, RuntimeDebugSpanStatus,
};
use noema_tasks::NewAgentRunItem;

use crate::{
    agent_execution::ExecutionRole,
    daemon::{
        RuntimeEventRegistry, agent_onboarding::AgentPromptIdentity, protocol::RuntimeError,
        task_run_context::TaskTerminalContract,
    },
};
use noema_providers::{
    GenerateInput, GenerateOptions, GenerateRequest, GenerateResponse, GenerateResponseStatus,
    NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, ProviderError,
    ProviderResponseContinuation, ProviderSelectionSnapshot, ProviderToolCapabilities, TokenUsage,
};

use super::{
    actor::RuntimeActor,
    continuation_context::ContinuationContext,
    local_tool_results::LocalToolResult,
    model_tools::{ModelTools, build_model_tools_for_role},
    progress::{ContinuationProgressTracker, DeterministicProgressStop},
    progress_audit::ProgressAuditDecision,
    runtime_debug::RuntimeDebugSpan,
    task_continuation::{
        add_usage, background_tool_instructions, build_task_finalization_prompt,
        is_task_terminal_tool, is_valid_terminal_tool, render_continuation_tool_names,
        task_requires_response_envelope, task_tool_result_transcript_payload,
        terminal_contract_tools, terminal_tool_instructions,
    },
    task_transcript::persisted_capability_arguments,
    tool_lifecycle::local_tool_calls,
    turn::{SuccessfulProviderTurn, current_runtime_environment},
};
use crate::daemon::prompts::build_role_tool_result_continuation_system_prompt;
use tokio_util::sync::CancellationToken;

/// Provider request for one background Planner, Executor, or Reviewer run.
#[derive(Debug, Clone)]
pub(crate) struct BackgroundTaskGenerateRequest {
    /// Durable run id used as the stateless provider conversation id.
    pub run_id: String,
    /// Durable task id used to route GraphQL detail updates.
    pub task_id: String,
    /// Active lease token fencing every durable run write.
    pub lease_token: String,
    /// Task generation captured when this run was claimed.
    pub task_generation: u64,
    /// Contract captured when this run was claimed.
    pub contract_id: Option<noema_tasks::TaskContractId>,
    /// Per-run cancellation propagated through provider and tool futures.
    pub cancellation: CancellationToken,
    /// Built-in agent identity that owns this run.
    pub agent_id: String,
    /// Stable human-friendly name for this conversation-history instance.
    pub instance_name: String,
    /// Role policy applied to advertised and dispatched tools.
    pub role: ExecutionRole,
    /// Complete immutable provider selection retained from the durable run.
    pub provider_selection: ProviderSelectionSnapshot,
    /// Immutable provider-independent execution-policy snapshot.
    pub execution_policy: noema_tasks::TaskExecutionPolicy,
    /// User/task prompt supplied to the provider.
    pub input: String,
    /// System instructions for the executor or reviewer contract.
    pub instructions: String,
    /// Run-specific terminal payload contract derived from admitted task state.
    pub terminal_contract: TaskTerminalContract,
    /// Runtime event registry for live task-detail refreshes.
    pub runtime_events: RuntimeEventRegistry,
}

impl BackgroundTaskGenerateRequest {
    pub(crate) fn work_run_fence(&self) -> noema_store::WorkRunFence {
        noema_store::WorkRunFence {
            run_id: self.run_id.clone(),
            lease_token: self.lease_token.clone(),
            task_generation: self.task_generation,
            contract_id: self.contract_id.clone(),
        }
    }
}

include!("background_task/generate.rs");
include!("background_task/finalize.rs");

fn should_stop_after_tool_results(results: &[LocalToolResult]) -> bool {
    results.iter().any(|result| result.has_uncertain_outcome())
}

fn propagate_compaction_result(result: Result<bool, ProviderError>) -> Result<(), RuntimeError> {
    result.map(|_| ()).map_err(RuntimeError::Provider)
}

fn is_wall_time_error(error: &RuntimeError) -> bool {
    matches!(
        error,
        RuntimeError::Protocol(message)
            if message == "task active wall-time safety ceiling reached"
    )
}

fn task_finalization_deadline(
    _run_deadline: tokio::time::Instant,
    now: tokio::time::Instant,
) -> tokio::time::Instant {
    const FINALIZATION_GRACE: Duration = Duration::from_secs(30);
    now + FINALIZATION_GRACE
}

fn binding_snapshot_for_specs(
    model_tools: &ModelTools,
    specs: &[noema_capabilities::ToolSpec],
) -> Result<noema_capabilities::CapabilityCatalogSnapshot, RuntimeError> {
    let mut builder = noema_capabilities::CapabilityCatalogBuilder::new();
    for spec in specs {
        let binding = model_tools
            .bindings
            .resolve(spec.name.as_str())
            .ok_or_else(|| {
                RuntimeError::Protocol(
                    "terminal capability binding is missing from the active catalog".to_string(),
                )
            })?
            .clone();
        builder.add(binding).map_err(|_| {
            RuntimeError::Protocol("terminal capability catalog is invalid".to_string())
        })?;
    }
    Ok(builder.build())
}

#[cfg(test)]
mod tests {
    use super::super::local_tool_results::LocalToolKind;
    use super::*;

    #[test]
    fn compaction_provider_error_is_preserved_for_task_failure_finalization() {
        let error = ProviderError::ProviderUnavailable {
            provider: "test-provider".to_string(),
            message: "continuation compaction failed".to_string(),
        };

        let error = propagate_compaction_result(Err(error)).expect_err("propagate error");
        assert!(matches!(error, RuntimeError::Provider(_)));
        assert!(error.to_string().contains("continuation compaction failed"));
    }

    #[test]
    fn background_prompt_contains_escaped_stable_instance_name() {
        let prompt =
            render_background_instance_identity("task instructions".to_string(), "Amber\nFinch");
        assert!(prompt.contains("Subagent instance identity:"));
        assert!(prompt.contains(r#"instance_name: "Amber\nFinch""#));
        assert!(prompt.contains("remains stable for this run"));
    }

    #[test]
    fn uncertain_tool_results_stop_background_model_continuation() {
        let result = LocalToolResult {
            call_id: None,
            provider_call_id: None,
            provider_name: None,
            name: "fixture.write".to_string(),
            arguments: serde_json::Value::Null,
            persisted: noema_capabilities::PersistedCapabilityPayload::omitted(),
            success: false,
            execution_payload: serde_json::json!({"error": "outcome_uncertain"}),
            payload: serde_json::json!({"error": "capability outcome is uncertain"}),
            requires_provider_continuation: false,
            blocked_action_id: None,
            blocked_authentication_id: None,
            blocked_outcome_uncertain: true,
            kind: LocalToolKind::Gateway,
        };

        assert!(should_stop_after_tool_results(&[result]));
    }
}
