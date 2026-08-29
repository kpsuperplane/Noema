//! Store-owned run fences and typed terminal envelopes.
//!
//! Runtime workers never mutate Work rows directly.  They submit one of these
//! validated envelopes to the command service, which applies the lease,
//! generation, role, and workflow predicates in one SQLite transaction.

use noema_tasks::{
    AgentRunRecord, RunStatus, SafeErrorCode, TaskComplexity, TaskGateKind, WorkDomainError,
};
use serde::{Deserialize, Serialize};

use crate::{NoemaStore, StoreError};

#[path = "task_controls.rs"]
mod claim;
#[path = "agent_runs/events.rs"]
mod progress;
#[path = "agent_runs/records.rs"]
pub(crate) mod rows;
#[path = "tasks/submissions.rs"]
mod terminals;

pub(crate) use terminals::report_expired_failure_tx;

impl NoemaStore {
    /// Load one run by its exact durable identity without applying a live task
    /// generation or lease fence.
    ///
    /// This read is intentionally historical: cancellation clears a run's
    /// lease and the task may advance generations, but Runtime still needs to
    /// observe the terminal run record that its worker owned.
    ///
    /// # Errors
    ///
    /// Returns an error when the historical run row cannot be decoded.
    pub async fn get_work_run_record(
        &self,
        run_id: &str,
    ) -> Result<Option<AgentRunRecord>, StoreError> {
        let run_id = run_id.trim().to_owned();
        if run_id.is_empty() {
            return Err(StoreError::Work(WorkDomainError::InvalidInput {
                field: "run_id",
                message: "run id cannot be blank".to_string(),
            }));
        }
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let run = rows::load_run_tx(&transaction, &run_id)?;
            transaction.commit()?;
            Ok(run)
        })
        .await
    }

    /// Attach the negotiated ACP session identity to one live fenced run.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid or stale run fence or session identity.
    pub async fn record_acp_session_id(
        &self,
        fence: &WorkRunFence,
        session_id: &str,
    ) -> Result<(), StoreError> {
        fence.validate().map_err(StoreError::Work)?;
        let session_id = session_id.trim().to_string();
        if session_id.is_empty() {
            return Err(StoreError::Work(WorkDomainError::InvalidInput {
                field: "run.acp_session_id",
                message: "ACP session id cannot be blank".to_string(),
            }));
        }
        self.with_connection(|connection| {
            let changed = connection.execute(
                "UPDATE agent_runs SET acp_session_id = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND lease_token = ?3 AND task_generation = ?4 AND execution_backend_kind = 'acp' AND status = 'running'",
                rusqlite::params![fence.run_id, session_id, fence.lease_token, fence.task_generation],
            )?;
            if changed == 1 {
                Ok(())
            } else {
                Err(StoreError::Work(WorkDomainError::RunFenced))
            }
        }).await
    }
}

/// Run lease and task-generation fence supplied with every worker write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkRunFence {
    /// Run being advanced.
    pub run_id: String,
    /// Opaque lease token issued by `claim_next_work_run`.
    pub lease_token: String,
    /// Task generation captured by the worker.
    pub task_generation: u64,
}

impl WorkRunFence {
    /// Validate shape-only fields before opening a transaction.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] when a fence identity is blank
    /// or its generation is zero.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.run_id.trim().is_empty() || self.lease_token.trim().is_empty() {
            return Err(WorkDomainError::InvalidInput {
                field: "run_fence",
                message: "run and lease token cannot be blank".to_string(),
            });
        }
        if self.task_generation == 0 {
            return Err(WorkDomainError::InvalidInput {
                field: "run_fence.task_generation",
                message: "generation must be positive".to_string(),
            });
        }
        Ok(())
    }
}

/// One atomic, additive observation of leased run progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkRunProgress {
    /// Exact lease, generation, and contract fence.
    pub fence: WorkRunFence,
    /// Provider that served this call; present exactly when provider calls increment.
    pub actual_provider_kind: Option<String>,
    /// Model that served this call; paired with `actual_provider_kind`.
    pub actual_model_profile: Option<String>,
    /// Number of provider calls completed by this observation.
    pub provider_call_count_delta: u32,
    /// Number of tool calls observed by this progress boundary.
    pub tool_call_count_delta: u32,
    /// Provider-reported input tokens added by this observation.
    pub input_tokens_delta: u64,
    /// Provider-reported cached input tokens added by this observation.
    pub cached_input_tokens_delta: u64,
    /// Provider-reported output tokens added by this observation.
    pub output_tokens_delta: u64,
    /// Active execution time added by this observation.
    pub active_milliseconds_delta: u64,
}

impl WorkRunProgress {
    /// Validate the observation shape before reading durable aggregate values.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] when the fence or bounded
    /// progress counters are invalid.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()?;
        let actual = match (&self.actual_provider_kind, &self.actual_model_profile) {
            (Some(provider), Some(model)) => {
                validate_actual(provider, "run_progress.actual_provider_kind", 128)?;
                validate_actual(model, "run_progress.actual_model_profile", 256)?;
                true
            }
            (None, None) => false,
            _ => {
                return Err(WorkDomainError::InvalidInput {
                    field: "run_progress.actual_provider",
                    message: "actual provider and model must be supplied together".to_string(),
                });
            }
        };
        if actual != (self.provider_call_count_delta > 0) {
            return Err(WorkDomainError::InvalidInput {
                field: "run_progress.provider_call_count_delta",
                message: "provider identity is required exactly when provider calls increment"
                    .to_string(),
            });
        }
        if self.cached_input_tokens_delta > self.input_tokens_delta {
            return Err(WorkDomainError::InvalidInput {
                field: "run_progress.cached_input_tokens_delta",
                message: "cached input tokens cannot exceed input tokens".to_string(),
            });
        }
        if self.provider_call_count_delta == 0
            && self.tool_call_count_delta == 0
            && self.input_tokens_delta == 0
            && self.cached_input_tokens_delta == 0
            && self.output_tokens_delta == 0
            && self.active_milliseconds_delta == 0
        {
            return Err(WorkDomainError::InvalidInput {
                field: "run_progress",
                message: "progress observation cannot be empty".to_string(),
            });
        }
        Ok(())
    }
}

fn validate_actual(
    value: &str,
    field: &'static str,
    max_len: usize,
) -> Result<(), WorkDomainError> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > max_len
        || value.chars().any(char::is_control)
    {
        Err(WorkDomainError::InvalidInput {
            field,
            message: "actual provider/model value is not canonical".to_string(),
        })
    } else {
        Ok(())
    }
}

/// Planner completion marker. The plan itself remains in mutable Task files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishPlanning {
    /// Run lease and generation fence.
    pub fence: WorkRunFence,
    /// Complexity selected for later execution runs.
    pub complexity: TaskComplexity,
}

impl FinishPlanning {
    /// Validate the run fence.
    ///
    /// # Errors
    ///
    /// Returns an error when the run fence is invalid.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()
    }
}

/// Executor completion marker. The result remains in mutable Task files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishExecution {
    /// Run lease and generation fence.
    pub fence: WorkRunFence,
}

impl FinishExecution {
    /// Validate the run fence.
    ///
    /// # Errors
    ///
    /// Returns an error when the run fence is invalid.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()
    }
}

/// Executor continuation marker. The next run resumes from current Task files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinueExecution {
    /// Run lease and generation fence.
    pub fence: WorkRunFence,
}

impl ContinueExecution {
    /// Validate the run fence.
    ///
    /// # Errors
    ///
    /// Returns an error when the run fence is invalid.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()
    }
}

/// Reviewer completion marker with current feedback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishReview {
    /// Run lease and generation fence.
    pub fence: WorkRunFence,
    /// Current workflow decision.
    pub decision: noema_tasks::TaskReviewVerdict,
    /// Concise feedback written to `REVIEW.md`.
    pub feedback: String,
    /// Whether approval sends a completion update to the human.
    pub notify_human: bool,
}

impl FinishReview {
    /// Validate the fence and feedback.
    ///
    /// # Errors
    ///
    /// Returns an error when the fence or feedback is invalid.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()?;
        if self.feedback.trim().is_empty() || self.feedback.len() > 20_000 {
            return Err(WorkDomainError::InvalidInput {
                field: "review.feedback",
                message: "review feedback must contain at most 20,000 bytes".to_string(),
            });
        }
        Ok(())
    }
}

/// Planner or Executor request for a human gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportTaskBlocked {
    /// Run lease and generation fence.
    pub fence: WorkRunFence,
    /// Clarification or Approval gate kind.
    pub gate_kind: TaskGateKind,
    /// Safe prompt shown to the human.
    pub prompt_markdown: String,
    /// Bounded supporting context.
    pub context_markdown: String,
    /// Optional direct answers shown to the human.
    pub suggested_answers: Vec<String>,
}

impl ReportTaskBlocked {
    /// Validate gate shape before the transaction checks current task/run role.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] when the fence, prompt, or
    /// context is malformed.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()?;
        if self.prompt_markdown.trim().is_empty()
            || !matches!(
                self.gate_kind,
                TaskGateKind::Clarification | TaskGateKind::Approval
            )
        {
            return Err(WorkDomainError::InvalidInput {
                field: "blocked_gate",
                message: "prompt and clarification/approval gate are required".to_string(),
            });
        }
        validate_suggested_answers(&self.suggested_answers, "blocked_gate")?;
        Ok(())
    }
}

fn validate_suggested_answers(
    answers: &[String],
    field: &'static str,
) -> Result<(), WorkDomainError> {
    if answers.len() > 8
        || answers
            .iter()
            .any(|answer| answer.trim().is_empty() || answer.chars().count() > 1_000)
    {
        return Err(WorkDomainError::InvalidInput {
            field,
            message: "at most eight nonblank answers of 1,000 characters are allowed".to_string(),
        });
    }
    Ok(())
}

/// Safe failure report used by lease-expiry and worker failure recovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportRunFailure {
    /// Run lease/generation fence.  Lease expiry may use an empty token only
    /// through the store-owned recovery path, never through this envelope.
    pub fence: WorkRunFence,
    /// Terminal/interrupted status to persist.
    pub status: RunStatus,
    /// Closed safe error code retained in the ledger.
    pub error_code: SafeErrorCode,
    /// Redacted bounded diagnostic, never provider payloads or credentials.
    pub error_message: Option<String>,
    /// Whether the same lineage may be retried automatically.
    pub retryable: bool,
}

impl ReportRunFailure {
    /// Validate the failure status and safe diagnostic envelope.
    ///
    /// # Errors
    ///
    /// Returns an error when the fence, terminal status, safe error code, or
    /// bounded diagnostic message is invalid.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()?;
        if !matches!(self.status, RunStatus::Interrupted | RunStatus::Failed) {
            return Err(WorkDomainError::InvalidInput {
                field: "run_failure.status",
                message: "failure status must be interrupted or failed".to_string(),
            });
        }
        if self
            .error_message
            .as_deref()
            .is_some_and(|message| message.len() > 1024)
        {
            return Err(WorkDomainError::InvalidInput {
                field: "run_failure.error_message",
                message: "error message is too long".to_string(),
            });
        }
        Ok(())
    }
}

/// Claimed run returned to a worker after a FIFO lease transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimedWorkRun {
    /// Full immutable/durable run projection.
    pub run: AgentRunRecord,
    /// Lease token the worker must echo on every write.
    pub lease_token: String,
}

/// One runtime terminal marker accepted by the service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkRunTerminal {
    /// Planner finished writing the current Task plan.
    FinishPlanning(FinishPlanning),
    /// Executor finished the current Task work.
    FinishExecution(FinishExecution),
    /// Executor requests another run without human action.
    ContinueExecution(ContinueExecution),
    /// Reviewer recorded the current decision and feedback.
    FinishReview(FinishReview),
    /// Planner or Executor asks for a human gate.
    Blocked(ReportTaskBlocked),
}

impl WorkRunTerminal {
    /// Validate the envelope before the service opens its transaction.
    ///
    /// # Errors
    ///
    /// Returns the role-specific terminal validation error for malformed input.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        match self {
            Self::FinishPlanning(value) => value.validate(),
            Self::FinishExecution(value) => value.validate(),
            Self::ContinueExecution(value) => value.validate(),
            Self::FinishReview(value) => value.validate(),
            Self::Blocked(value) => value.validate(),
        }
    }
}
