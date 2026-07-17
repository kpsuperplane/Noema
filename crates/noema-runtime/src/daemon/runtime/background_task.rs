//! Provider/tool continuation loop for supervised task runs.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use noema_tasks::NewAgentRunItem;

use crate::{
    agent_execution::ExecutionRole,
    daemon::{RuntimeEventRegistry, agent_onboarding::AgentPromptIdentity, protocol::RuntimeError},
};
use noema_providers::{
    GenerateInput, GenerateOptions, GenerateRequest, GenerateResponse, GenerateResponseStatus,
    NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, ProviderError,
    ProviderSelectionSnapshot, TokenUsage,
};

use super::{
    actor::RuntimeActor,
    continuation_context::ContinuationContext,
    local_tools::LocalToolResult,
    model_tools::{ModelTools, build_model_tools_for_role},
    progress::{ContinuationProgressTracker, DeterministicProgressStop},
    progress_audit::ProgressAuditDecision,
    task_continuation::{
        add_usage, background_tool_instructions, build_task_finalization_prompt,
        is_task_terminal_tool, is_valid_terminal_tool, render_continuation_tool_names,
        task_tool_result_transcript_payload, terminal_contract_tools, terminal_tool_instructions,
    },
    task_transcript::persisted_capability_arguments,
    tool_lifecycle::local_tool_calls,
    turn::{SuccessfulProviderTurn, current_runtime_environment},
};
use crate::daemon::prompts::build_role_tool_result_continuation_system_prompt;
use tokio_util::sync::CancellationToken;

/// Provider request for one background executor or reviewer run.
#[derive(Debug, Clone)]
pub(crate) struct BackgroundTaskGenerateRequest {
    /// Durable run id used as the stateless provider conversation id.
    pub run_id: String,
    /// Durable task id used to route GraphQL detail updates.
    pub task_id: String,
    /// Active lease token fencing every durable run write.
    pub lease_token: String,
    /// Per-run cancellation propagated through provider and tool futures.
    pub cancellation: CancellationToken,
    /// Built-in agent identity that owns this run.
    pub agent_id: String,
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
    /// Runtime event registry for live task-detail refreshes.
    pub runtime_events: RuntimeEventRegistry,
}

include!("background_task/generate.rs");
include!("background_task/finalize.rs");

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
#[path = "background_task/tests.rs"]
mod tests;
