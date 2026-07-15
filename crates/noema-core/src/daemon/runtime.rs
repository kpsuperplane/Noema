pub(in crate::daemon) mod actor;
mod background_task;
pub(crate) mod context_compaction;
mod context_window;
mod continuation_context;
mod conversation_state;
pub(in crate::daemon) mod handle;
pub(in crate::daemon) mod local_tools;
pub(crate) mod model_context;
mod model_context_ledger;
pub(crate) mod model_tools;
mod progress;
pub(crate) mod progress_audit;
mod prompt_context;
mod task_completion;
mod task_continuation;
mod task_transcript;
mod tasks;
mod tool_lifecycle;
pub(in crate::daemon) mod transcript_persistence;
pub(in crate::daemon) mod turn;
pub(crate) mod turn_timing;
mod web_tools;

pub(crate) use background_task::BackgroundTaskGenerateRequest;
pub use handle::RuntimeModelProvider;
pub(crate) use handle::{CodexRuntimeHandle, RuntimeProviderMap};

/// Structured context for one primary-agent completion report.
#[derive(Debug, Clone)]
pub(crate) struct TaskCompletionDeliveryRequest {
    /// Stable task event identity used for exactly-once delivery.
    pub(crate) delivery_id: String,
    /// Task being reported.
    pub(crate) task_id: String,
    /// Originating conversation.
    pub(crate) conversation_id: String,
    /// Source item that created the task, when available.
    pub(crate) source_item_id: Option<String>,
    /// Human-visible task title.
    pub(crate) title: String,
    /// Canonical task status.
    pub(crate) status: String,
    /// Original delegated request.
    pub(crate) request_markdown: String,
    /// Executor summary, when a submission exists.
    pub(crate) summary: Option<String>,
    /// Full executor result, when a submission exists.
    pub(crate) result_markdown: Option<String>,
    /// Approved governed artifact snapshots.
    pub(crate) artifacts: Vec<TaskCompletionArtifact>,
    /// Reviewer feedback, when a review exists.
    pub(crate) review_feedback: Option<String>,
    /// Reviewer criterion outcomes.
    pub(crate) criteria: Vec<TaskCompletionCriterion>,
    /// Task-level blocked/error detail.
    pub(crate) detail: Option<String>,
}

/// One artifact attached to a completed task report.
#[derive(Debug, Clone)]
pub(crate) struct TaskCompletionArtifact {
    pub(crate) artifact_id: String,
    pub(crate) artifact_version_id: String,
    pub(crate) title: String,
    pub(crate) artifact_kind: String,
    pub(crate) storage_kind: String,
    pub(crate) external_url: Option<String>,
    pub(crate) download_url: Option<String>,
    pub(crate) media_type: Option<String>,
}

/// One criterion included in a completion report context.
#[derive(Debug, Clone)]
pub(crate) struct TaskCompletionCriterion {
    /// Stable criterion id.
    pub(crate) criterion_id: String,
    /// Criterion outcome, if reviewed.
    pub(crate) outcome: Option<String>,
    /// Reviewer evidence.
    pub(crate) evidence: Option<String>,
    /// Reviewer feedback.
    pub(crate) feedback: Option<String>,
}
