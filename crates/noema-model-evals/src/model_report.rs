use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Configuration for one isolated local-model qualification worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelEvalConfig {
    /// Stable candidate id used in reports and the llama.cpp model alias.
    pub model_id: String,
    /// Verified GGUF artifact to load.
    pub model_path: PathBuf,
    /// Root containing Noema's pinned backend-specific llama.cpp runtime.
    pub runtime_root: PathBuf,
    /// Context window used consistently across candidates.
    pub context_window_tokens: u32,
    /// Per-generation timeout.
    pub timeout_seconds: u64,
    /// Model startup timeout.
    pub startup_timeout_seconds: u64,
    /// Whether to run the unscored near-context and repeated-turn resource probe.
    pub run_resource_probe: bool,
}

/// One sanitized tool call retained in an evaluation result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelEvalToolCall {
    /// Canonical Noema tool name.
    pub name: String,
    /// Model-produced tool payload.
    pub payload: Value,
}

/// Result for one deterministic model-sensitive Noema scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelEvalCaseResult {
    /// Stable scenario id.
    pub case_id: String,
    /// Noema product surface exercised by the scenario.
    pub category: String,
    /// Whether failure blocks model qualification.
    pub critical: bool,
    /// Whether every deterministic predicate passed.
    pub passed: bool,
    /// End-to-end provider latency.
    pub latency_ms: u64,
    /// Time until the first user-visible streaming delta, when emitted.
    pub first_visible_delta_ms: Option<u64>,
    /// Number of streamed user-visible characters.
    pub streamed_chars: usize,
    /// Prompt tokens reported by llama.cpp.
    pub input_tokens: Option<u64>,
    /// Generated tokens reported by llama.cpp.
    pub output_tokens: Option<u64>,
    /// Sanitized assistant text retained for human inspection.
    pub assistant_text: String,
    /// Sanitized structured tool calls retained for human inspection.
    pub tool_calls: Vec<ModelEvalToolCall>,
    /// Deterministic predicate failure or provider/runtime error.
    pub failure: Option<String>,
}

/// Runtime memory observed while the isolated llama-server executes the suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEvalRuntimeMemory {
    /// Platform-specific physical-memory metric used for the observation.
    pub metric: String,
    /// Runtime memory immediately after llama-server became healthy.
    pub ready_bytes: u64,
    /// Highest sampled runtime memory across all evaluation cases.
    pub peak_bytes: u64,
}

/// Unscored resource probe used after correctness qualification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEvalResourceProbe {
    /// Raw input-token target used for the near-context request.
    pub target_input_tokens: u32,
    /// Input tokens reported by llama.cpp for the near-context request.
    pub observed_input_tokens: Option<u64>,
    /// End-to-end latency of the near-context request.
    pub near_context_latency_ms: Option<u64>,
    /// Number of distinct short turns requested after the near-context request.
    pub steady_turns_requested: u32,
    /// Number of distinct short turns that completed successfully.
    pub steady_turns_completed: u32,
    /// Lowest post-turn resident set observed during the steady-turn sequence.
    pub post_turn_min_bytes: Option<u64>,
    /// Highest post-turn resident set observed during the steady-turn sequence.
    pub post_turn_max_bytes: Option<u64>,
    /// First tokenizer, generation, or memory-sampling failure, when present.
    pub failure: Option<String>,
}

/// Complete direct-provider qualification report for one GGUF candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelEvalReport {
    /// Stable candidate id.
    pub model_id: String,
    /// Pinned llama.cpp release used by the run.
    pub llama_cpp_release: String,
    /// Pinned llama.cpp commit used by the run.
    pub llama_cpp_commit: String,
    /// Backend that loaded successfully.
    pub backend: Option<String>,
    /// Time for the supervised runtime to become healthy.
    pub runtime_load_ms: u64,
    /// Resident-set memory observed after load and during generation.
    pub runtime_memory: Option<ModelEvalRuntimeMemory>,
    /// Optional unscored near-context and repeated-turn resource probe.
    pub resource_probe: Option<ModelEvalResourceProbe>,
    /// Runtime compatibility failure, distinct from model-quality failures.
    pub runtime_error: Option<String>,
    /// Ordered scenario results.
    pub cases: Vec<ModelEvalCaseResult>,
    /// Passed deterministic predicates.
    pub passed_cases: usize,
    /// Total deterministic scenarios attempted.
    pub total_cases: usize,
    /// Passed qualification-critical scenarios.
    pub passed_critical_cases: usize,
    /// Total qualification-critical scenarios.
    pub total_critical_cases: usize,
}

impl From<noema_runtime::eval_support::RuntimeEvalToolCall> for ModelEvalToolCall {
    fn from(call: noema_runtime::eval_support::RuntimeEvalToolCall) -> Self {
        Self {
            name: call.name,
            payload: call.payload,
        }
    }
}

impl From<noema_runtime::eval_support::RuntimeEvalCaseResult> for ModelEvalCaseResult {
    fn from(result: noema_runtime::eval_support::RuntimeEvalCaseResult) -> Self {
        Self {
            case_id: result.case_id,
            category: result.category,
            critical: result.critical,
            passed: result.passed,
            latency_ms: result.latency_ms,
            first_visible_delta_ms: result.first_visible_delta_ms,
            streamed_chars: result.streamed_chars,
            input_tokens: result.input_tokens,
            output_tokens: result.output_tokens,
            assistant_text: result.assistant_text,
            tool_calls: result.tool_calls.into_iter().map(Into::into).collect(),
            failure: result.failure,
        }
    }
}
