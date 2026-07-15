use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::provider::GenerateRequest;

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

pub(super) struct EvalCase {
    pub id: &'static str,
    pub category: &'static str,
    pub critical: bool,
    pub request: GenerateRequest,
    pub expectation: EvalExpectation,
}

pub(super) enum EvalExpectation {
    ExactFinalText(&'static str),
    StreamedExactText(&'static str),
    MultipleChoice,
    MemoryLookup,
    MemoryContinuation,
    ExecutorSubmission,
    ReviewerApproval,
    BlockedTask,
    ProgressAuditFinalize,
    WebSummary,
    ContextCompaction,
}
