//! Task-role prompts, typed terminal payloads, and durable continuation context.

use serde::Deserialize;

use crate::NoemaStore;

pub(super) fn format_executor_prompt(
    task: &crate::TaskRecord,
    criteria: &[crate::TaskValidationCriterion],
    revision: i64,
) -> String {
    let criteria = criteria
        .iter()
        .map(|criterion| {
            format!(
                "{}. criterion_id={}: {}{}",
                criterion.ordinal,
                criterion.criterion_id,
                criterion.description,
                criterion
                    .expected_evidence
                    .as_deref()
                    .map(|value| format!(" Evidence: {value}"))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Task ID: {}\nTask: {}\nRevision: {revision}\n\nRequest:\n{}\n\nValidation criteria:\n{criteria}\n\nUse each criterion_id exactly as shown, including any prefix, when calling task.submit_result. If you call task.inspect, use the exact Task ID above. Produce a complete, useful result. Address every criterion explicitly, then submit it through task.submit_result. Do not finish with ordinary assistant text.",
        task.task_id, task.title, task.request_markdown
    )
}

pub(super) fn format_reviewer_prompt(
    task: &crate::TaskRecord,
    submission: &crate::TaskSubmissionRecord,
    criteria: &[crate::TaskValidationCriterion],
) -> String {
    let criteria = criteria
        .iter()
        .map(|criterion| {
            format!(
                "criterion_id={}: {}",
                criterion.criterion_id, criterion.description
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Task ID: {}\n\nOriginal request:\n{}\n\nCriteria:\n{criteria}\n\nExecutor result:\n{}\n\nUse each criterion_id exactly as shown, including any prefix, when calling task.submit_review. If you call task.inspect, use the exact Task ID above. Be adversarial, include every criterion exactly once, and submit the typed verdict through task.submit_review. Do not return JSON as ordinary assistant text.",
        task.task_id, task.request_markdown, submission.result_markdown
    )
}

pub(super) async fn with_resume_context(
    store: &NoemaStore,
    run: &crate::AgentRunRecord,
    mut prompt: String,
) -> Result<String, String> {
    let mut lineage = Vec::new();
    let mut parent_id = run.parent_run_id.clone();
    while let Some(run_id) = parent_id {
        if lineage.len() >= 32 {
            break;
        }
        let Some(parent) = store
            .get_agent_run(&run_id)
            .await
            .map_err(|error| error.to_string())?
        else {
            break;
        };
        parent_id = parent.parent_run_id.clone();
        lineage.push(parent);
    }
    if lineage.is_empty() && run.resume_message.is_none() {
        return Ok(prompt);
    }
    lineage.reverse();
    let mut history = String::new();
    for ancestor in lineage {
        let items = store
            .list_agent_run_items(&ancestor.run_id)
            .await
            .map_err(|error| error.to_string())?;
        for item in items {
            use crate::store::AgentRunItemStatus;
            if !matches!(
                item.status,
                AgentRunItemStatus::Completed
                    | AgentRunItemStatus::Failed
                    | AgentRunItemStatus::Cancelled
                    | AgentRunItemStatus::Skipped
            ) {
                continue;
            }
            let rendered = match item.kind.as_str() {
                "assistant_output" | "progress_notice" | "context_checkpoint" => {
                    item.content_text.unwrap_or_default()
                }
                "tool_call" | "tool_result" | "failure" | "cancellation" => {
                    format!(
                        "{}: {}\n{}",
                        item.kind,
                        item.content_text.unwrap_or_default(),
                        item.payload
                    )
                }
                _ => continue,
            };
            if !rendered.trim().is_empty() {
                history.push_str(&format!(
                    "\n[run {} · round {}]\n{}\n",
                    ancestor.run_id, item.round_index, rendered
                ));
            }
        }
    }
    const RESUME_CONTEXT_CHAR_LIMIT: usize = 120_000;
    if history.chars().count() > RESUME_CONTEXT_CHAR_LIMIT {
        history = history
            .chars()
            .rev()
            .take(RESUME_CONTEXT_CHAR_LIMIT)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
    }
    prompt.push_str("\n\nDurable context from prior attempts:\n");
    prompt.push_str(history.trim());
    if let Some(message) = run.resume_message.as_deref() {
        prompt.push_str("\n\nHuman continuation message:\n");
        prompt.push_str(message);
    }
    Ok(prompt)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutorSubmissionResponse {
    pub(super) summary: String,
    pub(super) result_markdown: String,
    pub(super) criteria: Vec<ExecutorCriterionResponse>,
    #[serde(default)]
    pub(super) artifact_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutorCriterionResponse {
    pub(super) criterion_id: String,
    pub(super) evidence_markdown: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutorBlockedResponse {
    pub(super) question: String,
    pub(super) work_summary: String,
    #[serde(default)]
    pub(super) resume_context: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReviewerResponse {
    pub(super) overall_verdict: String,
    pub(super) overall_feedback: String,
    pub(super) criteria: Vec<ReviewerCriterionResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReviewerCriterionResponse {
    pub(super) criterion_id: String,
    pub(super) outcome: String,
    pub(super) evidence_markdown: Option<String>,
    pub(super) feedback: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{format_executor_prompt, format_reviewer_prompt};

    fn task() -> crate::TaskRecord {
        let model = crate::ModelConfigSnapshot::explicit(
            "codex",
            "provider_account:codex:default",
            "gpt-5.6-luna",
            None,
            Some("test".to_string()),
        );
        crate::TaskRecord {
            task_id: "task:test".to_string(),
            title: "Test task".to_string(),
            request_markdown: "Return a result".to_string(),
            complexity: crate::TaskComplexity::Simple,
            status: crate::TaskStatus::Queued,
            owner_human_id: "human:local".to_string(),
            source: crate::TaskSource::default(),
            created_by_agent_id: "agent:primary".to_string(),
            creation_tool_call_id: None,
            pool_entry_id: "pool:test".to_string(),
            executor_model: model.clone(),
            reviewer_model: model,
            revision_index: 0,
            max_review_rounds: 3,
            final_submission_id: None,
            latest_run_id: None,
            blocked_question: None,
            blocked_context: None,
            terminal_reason: None,
            error_code: None,
            error_message: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            completed_at: None,
        }
    }

    #[test]
    fn executor_prompt_preserves_task_and_criterion_ids() {
        let prompt = format_executor_prompt(
            &task(),
            &[crate::TaskValidationCriterion {
                criterion_id: "criterion:abc".to_string(),
                ordinal: 1,
                description: "The result is present".to_string(),
                expected_evidence: None,
            }],
            0,
        );

        assert!(prompt.contains("Task ID: task:test"));
        assert!(prompt.contains("criterion_id=criterion:abc"));
        assert!(prompt.contains("including any prefix"));
    }

    #[test]
    fn reviewer_prompt_preserves_task_and_criterion_ids() {
        let prompt = format_reviewer_prompt(
            &task(),
            &crate::TaskSubmissionRecord {
                submission_id: "submission:test".to_string(),
                task_id: "task:test".to_string(),
                executor_run_id: "run:test".to_string(),
                revision_index: 0,
                summary: "A result".to_string(),
                result_markdown: "Result".to_string(),
                criteria: Vec::new(),
                created_at: "now".to_string(),
            },
            &[crate::TaskValidationCriterion {
                criterion_id: "criterion:abc".to_string(),
                ordinal: 1,
                description: "The result is present".to_string(),
                expected_evidence: None,
            }],
        );

        assert!(prompt.contains("Task ID: task:test"));
        assert!(prompt.contains("criterion_id=criterion:abc"));
        assert!(prompt.contains("including any prefix"));
    }
}
