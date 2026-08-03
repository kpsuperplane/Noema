use noema_workspaces::{ProjectId, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{
    TaskContractId, TaskGateId, TaskId, WorkDomainError, WorkflowId, WorkflowStageId,
    error::invalid_input,
    validation::{optional as normalize_optional, required},
};

/// Maximum number of user-facing conversation messages retained as execution
/// authorization context.
pub const TASK_AUTHORIZATION_CONTEXT_MAX_MESSAGES: usize = 7;

string_enum! {
/// Authorship role carried by one conversation authorization-context message.
pub enum TaskAuthorizationMessageRole, "task.authorization_context.message.role" {
    /// Authenticated human authority.
    Human => "human",
    /// Generated context that may resolve later human references but creates no authority.
    Assistant => "assistant",
}
}

/// One exact durable conversation message retained for later action review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskAuthorizationMessage {
    /// Durable source item identity.
    pub item_id: String,
    /// Server-derived authorship role.
    pub role: TaskAuthorizationMessageRole,
    /// Exact user-facing message text.
    pub text: String,
}

/// Human authority captured with a task and reused by governed action review.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TaskAuthorizationContext {
    /// Bounded primary-conversation excerpt ending at an authenticated human item.
    ConversationExcerpt {
        /// Oldest-to-newest exact messages.
        messages: Vec<TaskAuthorizationMessage>,
    },
    /// Authenticated task body submitted through Work UI.
    ManualTaskBody {
        /// Human-authored task title.
        title: String,
        /// Human-authored task description.
        description_markdown: String,
    },
    /// No authenticated human authority is attached.
    #[default]
    None,
}

impl TaskAuthorizationContext {
    /// Validate bounded context without rewriting exact source text.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError`] when an excerpt is empty, over the message
    /// limit, does not end with a human message, or contains invalid fields.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        match self {
            Self::ConversationExcerpt { messages } => {
                if messages.is_empty() || messages.len() > TASK_AUTHORIZATION_CONTEXT_MAX_MESSAGES {
                    return Err(invalid_input(
                        "task.authorization_context.messages",
                        "conversation excerpt must contain between one and seven messages",
                    ));
                }
                for message in messages {
                    if message.item_id.trim().is_empty() || message.text.trim().is_empty() {
                        return Err(invalid_input(
                            "task.authorization_context.messages",
                            "message item id and text are required",
                        ));
                    }
                }
                if messages.last().map(|message| message.role)
                    != Some(TaskAuthorizationMessageRole::Human)
                {
                    return Err(invalid_input(
                        "task.authorization_context.messages",
                        "conversation excerpt must end with authenticated human authority",
                    ));
                }
                Ok(())
            }
            Self::ManualTaskBody { title, .. } => {
                required(title, "task.authorization_context.title").map(drop)
            }
            Self::None => Ok(()),
        }
    }
}

string_enum! {
/// Durable source/provenance classification for a captured task.
pub enum TaskSourceKind, "task.provenance.source_kind" {
    /// Captured from the primary conversation without execution authorization.
    ChatCapture => "chat_capture",
    /// Delegated from the primary conversation for asynchronous execution.
    ChatDelegate => "chat_delegate",
    /// Created through Work UI.
    WorkUi => "work_ui",
    /// Created by a system/reconciliation action.
    System => "system",
}
}

#[allow(
    clippy::derivable_impls,
    reason = "the shared enum macro owns common derives"
)]
impl Default for TaskSourceKind {
    fn default() -> Self {
        Self::System
    }
}

/// Durable provenance linking a task to its source conversation/turn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskProvenance {
    pub source_kind: TaskSourceKind,
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub source_tool_call_id: Option<String>,
    pub created_by_actor_id: String,
}

impl TaskProvenance {
    /// Normalize optional references and validate the actor identity.
    /// # Errors
    /// Returns [`WorkDomainError`] when the creating actor identity is blank.
    pub fn normalized(&self) -> Result<Self, WorkDomainError> {
        let created_by_actor_id = required(
            &self.created_by_actor_id,
            "task.provenance.created_by_actor_id",
        )?;
        Ok(Self {
            source_kind: self.source_kind,
            conversation_id: normalize_optional(self.conversation_id.as_deref()),
            turn_id: normalize_optional(self.turn_id.as_deref()),
            item_id: normalize_optional(self.item_id.as_deref()),
            source_tool_call_id: normalize_optional(self.source_tool_call_id.as_deref()),
            created_by_actor_id,
        })
    }
}

/// Durable current task projection.  Stage is the only task-level workflow state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskRecord {
    pub task_id: TaskId,
    pub workspace_id: WorkspaceId,
    pub project_id: Option<ProjectId>,
    pub workflow_id: WorkflowId,
    pub stage_id: WorkflowStageId,
    pub title: String,
    pub description_markdown: String,
    pub authorization_context: TaskAuthorizationContext,
    pub provenance: TaskProvenance,
    pub generation: u64,
    pub revision: u64,
    pub current_contract_id: Option<TaskContractId>,
    pub active_gate_id: Option<TaskGateId>,
    pub latest_run_id: Option<String>,
    pub latest_submission_id: Option<String>,
    pub latest_review_id: Option<String>,
    pub completed_submission_id: Option<String>,
    pub scheduled_for: Option<i64>,
    pub schedule_time_zone: Option<String>,
    pub missed_run_policy: Option<crate::MissedRunPolicy>,
    pub recurrence_id: Option<crate::TaskRecurrenceId>,
    pub recurrence_revision: Option<u64>,
    pub recurrence_scheduled_for: Option<i64>,
    pub queued_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub cancelled_at: Option<String>,
}

impl TaskRecord {
    /// Normalize and retain task-owned text/provenance instead of discarding
    /// the normalized values after validation.
    /// # Errors
    /// Returns [`WorkDomainError`] when the creating actor identity is blank.
    pub fn normalized(&self) -> Result<Self, WorkDomainError> {
        let mut normalized = self.clone();
        normalized.title = required(&normalized.title, "task.title")?;
        normalized.description_markdown = normalized.description_markdown.trim().to_string();
        normalized.authorization_context.validate()?;
        normalized.provenance = normalized.provenance.normalized()?;
        if normalized.generation == 0 || normalized.revision == 0 {
            return Err(invalid_input(
                "task",
                "generation and revision must be positive",
            ));
        }
        if normalized.created_at.trim().is_empty() || normalized.updated_at.trim().is_empty() {
            return Err(invalid_input(
                "task.timestamp",
                "timestamps cannot be blank",
            ));
        }
        if normalized.scheduled_for.is_some()
            != (normalized.schedule_time_zone.is_some() && normalized.missed_run_policy.is_some())
        {
            return Err(invalid_input(
                "task.schedule",
                "scheduledFor, timezone, and missed-run policy must be present together",
            ));
        }
        if normalized.recurrence_id.is_some()
            != (normalized.recurrence_revision.is_some()
                && normalized.recurrence_scheduled_for.is_some())
        {
            return Err(invalid_input(
                "task.recurrence",
                "recurrence provenance fields must be present together",
            ));
        }
        Ok(normalized)
    }

    /// Validate task-owned invariants without consulting persistence.
    /// # Errors
    /// Returns [`WorkDomainError`] under the same conditions as [`Self::normalized`].
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.normalized().map(|_| ())
    }
}
