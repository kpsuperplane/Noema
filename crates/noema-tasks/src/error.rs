use thiserror::Error;

/// Stable, safe failure vocabulary shared by Work commands and projections.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkDomainError {
    /// The requested object is absent or outside the actor's visible scope.
    #[error("work is unavailable")]
    WorkUnavailable,
    /// An optimistic row revision no longer matches.
    #[error("work revision is stale")]
    StaleRevision,
    /// A mutable Task document changed after the human loaded it.
    #[error("task document is stale")]
    StaleDocument,
    /// A task execution generation no longer matches.
    #[error("work generation is stale")]
    StaleGeneration,
    /// The semantic operation is not legal from the current stage.
    #[error("invalid work transition")]
    InvalidTransition,
    /// A stage and workflow do not belong together.
    #[error("workflow and stage do not match")]
    WorkflowMismatch,
    /// A new or moved Inbox task targets an archived project.
    #[error("project is archived")]
    ProjectArchived,
    /// Waiting resolution did not identify the current open gate.
    #[error("the current open task gate is required")]
    GateRequired,
    /// An operation cannot proceed while a gate remains open.
    #[error("the task gate is unresolved")]
    GateUnresolved,
    /// Another automated review round would exceed the configured bound.
    #[error("the review-round limit has been reached")]
    ReviewLimitReached,
    /// Provider/model/policy selection was unavailable.
    #[error("task configuration is unavailable")]
    ConfigurationUnavailable,
    /// A worker lease or generation fence rejected a terminal write.
    #[error("the run is fenced")]
    RunFenced,
    /// An idempotency key was reused with different command content.
    #[error("the idempotency key conflicts with a prior command")]
    IdempotencyConflict,
    /// A required field, identifier, criterion, bound, or closed enum was malformed.
    #[error("invalid work input for {field}: {message}")]
    InvalidInput {
        /// Field or closed vocabulary being validated.
        field: &'static str,
        /// Safe validation detail.
        message: String,
    },
}

impl WorkDomainError {
    /// Return the stable API/store error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::WorkUnavailable => "work_unavailable",
            Self::StaleRevision => "stale_revision",
            Self::StaleDocument => "stale_document",
            Self::StaleGeneration => "stale_generation",
            Self::InvalidTransition => "invalid_transition",
            Self::WorkflowMismatch => "workflow_mismatch",
            Self::ProjectArchived => "project_archived",
            Self::GateRequired => "gate_required",
            Self::GateUnresolved => "gate_unresolved",
            Self::ReviewLimitReached => "review_limit_reached",
            Self::ConfigurationUnavailable => "configuration_unavailable",
            Self::RunFenced => "run_fenced",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::InvalidInput { .. } => "invalid_input",
        }
    }
}

pub(crate) fn invalid_input(field: &'static str, message: impl Into<String>) -> WorkDomainError {
    WorkDomainError::InvalidInput {
        field,
        message: message.into(),
    }
}
