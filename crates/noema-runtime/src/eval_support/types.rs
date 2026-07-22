use noema_providers::GenerateRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One sanitized tool call retained from a runtime evaluation scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeEvalToolCall {
    /// Canonical Noema tool name.
    pub name: String,
    /// Model-produced tool payload.
    pub payload: Value,
}

/// Result of one deterministic runtime-sensitive evaluation scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeEvalCaseResult {
    /// Stable scenario identifier.
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
    /// Prompt tokens reported by the provider.
    pub input_tokens: Option<u64>,
    /// Generated tokens reported by the provider.
    pub output_tokens: Option<u64>,
    /// Bounded assistant text retained for human inspection.
    pub assistant_text: String,
    /// Sanitized structured tool calls retained for human inspection.
    pub tool_calls: Vec<RuntimeEvalToolCall>,
    /// Deterministic predicate failure or provider error.
    pub failure: Option<String>,
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
    AgentNameUpdate,
    MemoryLookup,
    MemoryContinuation,
    SimplePlannerPlan,
    ExecutorSubmission,
    ReviewerApproval,
    ReviewerRequestChanges,
    BlockedTask,
    ProgressAuditFinalize,
    WebSummary,
    ContextCompaction,
}
