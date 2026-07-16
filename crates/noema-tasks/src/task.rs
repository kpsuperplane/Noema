use std::collections::HashSet;

use noema_providers::ProviderSelectionSnapshot;
use serde::{Deserialize, Serialize};

use crate::{
    DEFAULT_TASK_MAX_REVIEW_ROUNDS, NewTaskValidationCriterion, TaskComplexity, TaskDomainError,
    TaskStatus,
};

/// Provenance linking a task to its originating conversation turn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSource {
    /// Source conversation, if the task was delegated from chat.
    pub conversation_id: Option<String>,
    /// Source turn, if known.
    pub turn_id: Option<String>,
    /// Source transcript item, if known.
    pub item_id: Option<String>,
}

/// Input for an atomic task plus first executor-run transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTask {
    /// Optional caller-supplied task id.
    pub task_id: Option<String>,
    /// Human-visible short title.
    pub title: String,
    /// Normalized Markdown request for the executor.
    pub request_markdown: String,
    /// Complexity tier used to select the pool entry.
    pub complexity: TaskComplexity,
    /// Human owning the task.
    pub owner_human_id: String,
    /// Source conversation provenance.
    pub source: TaskSource,
    /// Agent that created the task.
    pub created_by_agent_id: String,
    /// Provider call id that performed delegation, when available.
    pub creation_tool_call_id: Option<String>,
    /// Exact pool entry chosen by the primary agent.
    pub pool_entry_id: String,
    /// Immutable executor model snapshot.
    pub executor_model: ProviderSelectionSnapshot,
    /// Immutable reviewer model-request snapshot.
    pub reviewer_model: ProviderSelectionSnapshot,
    /// Initial review-round bound; defaults to three when omitted.
    pub max_review_rounds: Option<i64>,
    /// Immutable criteria checked by the reviewer.
    pub criteria: Vec<NewTaskValidationCriterion>,
}

impl NewTask {
    /// Normalize fields and validate criteria before entering persistence.
    ///
    /// # Errors
    ///
    /// Returns [`TaskDomainError`] when the task is malformed.
    pub fn normalized(&self) -> Result<Self, TaskDomainError> {
        let title = required(&self.title, "title")?;
        let request_markdown = required(&self.request_markdown, "request_markdown")?;
        let owner_human_id = required(&self.owner_human_id, "owner_human_id")?;
        let created_by_agent_id = required(&self.created_by_agent_id, "created_by_agent_id")?;
        let pool_entry_id = required(&self.pool_entry_id, "pool_entry_id")?;
        let max_review_rounds = self
            .max_review_rounds
            .unwrap_or(DEFAULT_TASK_MAX_REVIEW_ROUNDS);
        if max_review_rounds < 1 {
            return Err(TaskDomainError::InvalidReviewRoundLimit(max_review_rounds));
        }
        if self.criteria.is_empty() {
            return Err(TaskDomainError::NoCriteria);
        }
        let mut criteria = self.criteria.clone();
        criteria.sort_by_key(|criterion| criterion.ordinal);
        let mut previous_ordinal = None;
        let mut descriptions = HashSet::with_capacity(criteria.len());
        for criterion in &mut criteria {
            if criterion.ordinal < 1 {
                return Err(TaskDomainError::InvalidCriterionOrdinal(criterion.ordinal));
            }
            if previous_ordinal == Some(criterion.ordinal) {
                return Err(TaskDomainError::DuplicateCriterionOrdinal(
                    criterion.ordinal,
                ));
            }
            previous_ordinal = Some(criterion.ordinal);
            let description = required(&criterion.description, "criterion.description")?;
            if !descriptions.insert(description.clone()) {
                return Err(TaskDomainError::DuplicateCriterionDescription(description));
            }
            criterion.description = description;
            criterion.expected_evidence = normalize_optional(criterion.expected_evidence.as_ref());
        }

        Ok(Self {
            task_id: normalize_optional(self.task_id.as_ref()),
            title,
            request_markdown,
            complexity: self.complexity,
            owner_human_id,
            source: TaskSource {
                conversation_id: normalize_optional(self.source.conversation_id.as_ref()),
                turn_id: normalize_optional(self.source.turn_id.as_ref()),
                item_id: normalize_optional(self.source.item_id.as_ref()),
            },
            created_by_agent_id,
            creation_tool_call_id: normalize_optional(self.creation_tool_call_id.as_ref()),
            pool_entry_id,
            executor_model: self.executor_model.normalized_for_persistence()?,
            reviewer_model: self.reviewer_model.normalized_for_persistence()?,
            max_review_rounds: Some(max_review_rounds),
            criteria,
        })
    }
}

/// Persisted task projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    /// Stable task id.
    pub task_id: String,
    /// Human-visible title.
    pub title: String,
    /// Immutable normalized request.
    pub request_markdown: String,
    /// Complexity tier.
    pub complexity: TaskComplexity,
    /// Current workflow state.
    pub status: TaskStatus,
    /// Owning human.
    pub owner_human_id: String,
    /// Source conversation/turn/item provenance.
    pub source: TaskSource,
    /// Creating agent.
    pub created_by_agent_id: String,
    /// Delegation tool call id.
    pub creation_tool_call_id: Option<String>,
    /// Selected pool entry.
    pub pool_entry_id: String,
    /// Executor model snapshot.
    pub executor_model: ProviderSelectionSnapshot,
    /// Reviewer model snapshot.
    pub reviewer_model: ProviderSelectionSnapshot,
    /// Current revision index.
    pub revision_index: i64,
    /// Maximum reviewed submissions.
    pub max_review_rounds: i64,
    /// Approved submission id.
    pub final_submission_id: Option<String>,
    /// Latest run id.
    pub latest_run_id: Option<String>,
    /// Human-facing question that must be answered before resuming.
    pub blocked_question: Option<String>,
    /// Executor summary retained while waiting for human input.
    pub blocked_context: Option<String>,
    /// Terminal reason.
    pub terminal_reason: Option<String>,
    /// Safe error code.
    pub error_code: Option<String>,
    /// Safe error message.
    pub error_message: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// Completion timestamp.
    pub completed_at: Option<String>,
}

fn required(value: &str, field: &'static str) -> Result<String, TaskDomainError> {
    let value = value.trim();
    if value.is_empty() {
        Err(TaskDomainError::EmptyField(field))
    } else {
        Ok(value.to_string())
    }
}

fn normalize_optional(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use noema_providers::{ProviderInstanceKey, ProviderSelectionSnapshot};

    use super::*;

    fn task_with(criteria: Vec<NewTaskValidationCriterion>) -> NewTask {
        NewTask {
            task_id: None,
            title: "Example".to_string(),
            request_markdown: "Do the thing".to_string(),
            complexity: TaskComplexity::Simple,
            owner_human_id: "human:local".to_string(),
            source: TaskSource::default(),
            created_by_agent_id: "agent:primary".to_string(),
            creation_tool_call_id: None,
            pool_entry_id: "pool:1".to_string(),
            executor_model: ProviderSelectionSnapshot::explicit(
                "codex",
                "provider_account:codex:default",
                "gpt-5.5",
                None,
                None,
            ),
            reviewer_model: ProviderSelectionSnapshot::provider_default(
                "codex",
                "provider_account:codex:default",
                None,
                None,
            ),
            max_review_rounds: None,
            criteria,
        }
    }

    #[test]
    fn task_normalization_rejects_duplicate_criteria() {
        let task = task_with(vec![
            NewTaskValidationCriterion {
                criterion_id: None,
                ordinal: 1,
                description: "same".to_string(),
                expected_evidence: None,
            },
            NewTaskValidationCriterion {
                criterion_id: None,
                ordinal: 2,
                description: "same".to_string(),
                expected_evidence: None,
            },
        ]);
        assert!(matches!(
            task.normalized(),
            Err(TaskDomainError::DuplicateCriterionDescription(_))
        ));
    }

    #[test]
    fn task_normalization_rejects_unpersistable_provider_instance_identity() {
        let mut task = task_with(vec![NewTaskValidationCriterion {
            criterion_id: None,
            ordinal: 1,
            description: "Result exists".to_string(),
            expected_evidence: None,
        }]);
        task.executor_model.provider_instance_key =
            Some(ProviderInstanceKey::new("openai:default:1").expect("valid transient key"));

        assert!(matches!(
            task.normalized(),
            Err(TaskDomainError::Model(
                noema_providers::ProviderSelectionError::DurableInstanceKeyUnsupported
            ))
        ));
    }
}
