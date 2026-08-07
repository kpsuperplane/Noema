use noema_providers::GenerateRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stable model setting represented by the runtime evaluation suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEvalRole {
    /// Foreground primary-agent model.
    Primary,
    /// Simple task planner and executor pool.
    TaskSimple,
    /// Medium task planner and executor pool.
    TaskMedium,
    /// Difficult task planner and executor pool.
    TaskDifficult,
    /// Independent task reviewer model.
    TaskReviewer,
    /// Web-page summarizer model.
    WebFetchSummarizer,
    /// Tool-continuation progress-audit model.
    ToolProgressAudit,
    /// Governed-action review model.
    ActionReviewer,
    /// Native Markdown memory-consolidation model.
    MemoryConsolidation,
}

impl RuntimeEvalRole {
    /// Every supported model setting in stable report order.
    pub const ALL: &[Self] = &[
        Self::Primary,
        Self::TaskSimple,
        Self::TaskMedium,
        Self::TaskDifficult,
        Self::TaskReviewer,
        Self::WebFetchSummarizer,
        Self::ToolProgressAudit,
        Self::ActionReviewer,
        Self::MemoryConsolidation,
    ];

    /// Stable report identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::TaskSimple => "task_simple",
            Self::TaskMedium => "task_medium",
            Self::TaskDifficult => "task_difficult",
            Self::TaskReviewer => "task_reviewer",
            Self::WebFetchSummarizer => "web_fetch_summarizer",
            Self::ToolProgressAudit => "tool_progress_audit",
            Self::ActionReviewer => "action_reviewer",
            Self::MemoryConsolidation => "memory_consolidation",
        }
    }
}

impl std::fmt::Display for RuntimeEvalRole {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

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
    /// Model setting exercised by the scenario.
    pub role: RuntimeEvalRole,
    /// Noema product surface exercised by the scenario.
    pub category: String,
    /// Whether failure blocks model qualification.
    pub critical: bool,
    /// Whether every deterministic predicate passed.
    pub passed: bool,
    /// Provider identifier returned with the completed response.
    pub response_provider: Option<String>,
    /// Model identifier returned with the completed response.
    pub response_model: Option<String>,
    /// End-to-end provider latency.
    pub latency_ms: u64,
    /// Time until the first user-visible streaming delta, when emitted.
    pub first_visible_delta_ms: Option<u64>,
    /// Number of streamed user-visible characters.
    pub streamed_chars: usize,
    /// Prompt tokens reported by the provider.
    pub input_tokens: Option<u64>,
    /// Prompt tokens served from the provider cache, when reported.
    pub cached_input_tokens: Option<u64>,
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
    pub role: RuntimeEvalRole,
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
    MemoryPageRead {
        path: &'static str,
        id: &'static str,
    },
    MemoryContinuation,
    DiscoveryBeforeExternalWrite,
    GroundedExternalWrite {
        start: &'static str,
        end: &'static str,
    },
    SimplePlannerPlan,
    ExecutorSubmission(ExecutorScenario),
    ReviewerApproval,
    ReviewerRequestChanges,
    BlockedTask,
    ProgressAuditFinalize,
    WebSummary,
    ContextCompaction,
    ActionReviewer,
    MemoryConsolidation,
}

#[derive(Clone, Copy)]
pub(super) enum ExecutorScenario {
    SimpleRecommendation,
    MediumComparison,
    DifficultRanking,
}
