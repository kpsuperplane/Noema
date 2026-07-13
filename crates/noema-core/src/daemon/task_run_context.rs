//! Task-role prompts, typed terminal payloads, and durable continuation context.

use serde::Deserialize;

use crate::NoemaStore;

const RUN_LINEAGE_LIMIT: usize = 32;
const HUMAN_CONTEXT_VALUE_CHAR_LIMIT: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TaskHumanContinuationContext {
    pub(super) question: Option<String>,
    pub(super) answer: String,
}

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
        "Task ID: {}\nTask: {}\nRevision: {revision}\n\nRequest:\n{}\n\nValidation criteria:\n{criteria}\n\nUse artifact.create_local_file once per requested file and include every returned artifact_id in task.submit_result. Use each criterion_id exactly as shown, including any prefix, when calling task.submit_result. If you call task.inspect, use the exact Task ID above. Produce a complete, useful result. Address every criterion explicitly, then submit it through task.submit_result. Do not finish with ordinary assistant text.",
        task.task_id, task.title, task.request_markdown
    )
}

pub(super) fn format_reviewer_prompt(
    task: &crate::TaskRecord,
    submission: &crate::TaskSubmissionRecord,
    criteria: &[crate::TaskValidationCriterion],
    human_context: &[TaskHumanContinuationContext],
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
    let artifacts = if submission.artifacts.is_empty() {
        "None".to_string()
    } else {
        submission
            .artifacts
            .iter()
            .map(|linked| {
                format!(
                    "{}. artifact_id={} version_id={} title={} kind={} media_type={}",
                    linked.ordinal,
                    linked.artifact.artifact_id,
                    linked.version.artifact_version_id,
                    linked.artifact.title,
                    linked.artifact.artifact_kind,
                    linked.version.media_type.as_deref().unwrap_or("unknown"),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let human_context = if human_context.is_empty() {
        "None".to_string()
    } else {
        human_context
            .iter()
            .enumerate()
            .map(|(index, context)| {
                let question = context
                    .question
                    .as_deref()
                    .map(|question| {
                        format!(
                            "Question: {}\n   ",
                            bounded_text(question, HUMAN_CONTEXT_VALUE_CHAR_LIMIT)
                        )
                    })
                    .unwrap_or_default();
                format!(
                    "{}. {}Human answer: {}",
                    index + 1,
                    question,
                    bounded_text(&context.answer, HUMAN_CONTEXT_VALUE_CHAR_LIMIT)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "Task ID: {}\n\nOriginal request:\n{}\n\nHuman clarifications received during execution:\n{human_context}\n\nCriteria:\n{criteria}\n\nExecutor result:\n{}\n\nSubmitted artifacts:\n{artifacts}\n\nHuman answers are authoritative task context and may refine or supersede the original request or written criterion wording. Interpret every criterion in light of those answers, while still including every criterion_id exactly once and explaining any affected judgment. Use task.read_artifact with the exact artifact_id when artifact contents affect a criterion. Use each criterion_id exactly as shown, including any prefix, when calling task.submit_review. If you call task.inspect, use the exact Task ID above. Be adversarial and submit the typed verdict through task.submit_review. Do not return JSON as ordinary assistant text.",
        task.task_id, task.request_markdown, submission.result_markdown
    )
}

pub(super) async fn human_continuation_context_for_submission(
    store: &NoemaStore,
    submission: &crate::TaskSubmissionRecord,
) -> Result<Vec<TaskHumanContinuationContext>, String> {
    let runs = store
        .list_agent_runs_for_task(&submission.task_id)
        .await
        .map_err(|error| error.to_string())?;
    let mut context = Vec::new();
    let mut found_executor = false;
    for run in runs {
        if let Some(answer) = run.resume_message.as_deref() {
            let question = match run.parent_run_id.as_deref() {
                Some(parent_run_id) => store
                    .task_blocking_question_for_run(&run.task_id, parent_run_id)
                    .await
                    .map_err(|error| error.to_string())?,
                None => None,
            };
            context.push(TaskHumanContinuationContext {
                question,
                answer: answer.to_string(),
            });
        }
        if run.run_id == submission.executor_run_id {
            found_executor = true;
            break;
        }
    }
    if !found_executor {
        return Err("submission executor run disappeared before review".to_string());
    }
    Ok(context)
}

pub(super) async fn with_resume_context(
    store: &NoemaStore,
    run: &crate::AgentRunRecord,
    mut prompt: String,
) -> Result<String, String> {
    let lineage = load_run_lineage(store, run).await?;
    if lineage.is_empty() && run.resume_message.is_none() {
        return Ok(prompt);
    }
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
        prompt.push_str("\n\nHuman continuation answer (authoritative task clarification that may refine the request or validation criteria):\n");
        prompt.push_str(message);
    }
    Ok(prompt)
}

async fn load_run_lineage(
    store: &NoemaStore,
    run: &crate::AgentRunRecord,
) -> Result<Vec<crate::AgentRunRecord>, String> {
    let mut lineage = Vec::new();
    let mut parent_id = run.parent_run_id.clone();
    while let Some(run_id) = parent_id {
        if lineage.len() >= RUN_LINEAGE_LIMIT {
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
    lineage.reverse();
    Ok(lineage)
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut bounded = value.chars().take(max_chars).collect::<String>();
    bounded.push_str("\n[truncated]");
    bounded
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
    use super::{
        TaskHumanContinuationContext, format_executor_prompt, format_reviewer_prompt,
        human_continuation_context_for_submission,
    };

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
                artifacts: Vec::new(),
                created_at: "now".to_string(),
            },
            &[crate::TaskValidationCriterion {
                criterion_id: "criterion:abc".to_string(),
                ordinal: 1,
                description: "The result is present".to_string(),
                expected_evidence: None,
            }],
            &[TaskHumanContinuationContext {
                question: Some("Should the report include archived data?".to_string()),
                answer: "No, evaluate only current data.".to_string(),
            }],
        );

        assert!(prompt.contains("Task ID: task:test"));
        assert!(prompt.contains("criterion_id=criterion:abc"));
        assert!(prompt.contains("Should the report include archived data?"));
        assert!(prompt.contains("No, evaluate only current data."));
        assert!(prompt.contains("may refine or supersede"));
        assert!(prompt.contains("including any prefix"));
    }

    #[tokio::test]
    async fn reviewer_context_pairs_executor_question_with_human_answer() {
        let store = crate::store::tests::test_store().await;
        let (task, run) = crate::store::tests::seed_task(&store, "Clarified task").await;
        store
            .claim_next_agent_run("worker:test", "lease:blocked", 120)
            .await
            .expect("claim")
            .expect("executor");
        store
            .transition_agent_run(
                &run.run_id,
                crate::RunStatus::Running,
                Some("lease:blocked"),
                None,
            )
            .await
            .expect("running");
        store
            .transition_task(&task.task_id, crate::TaskStatus::Executing, None)
            .await
            .expect("executing");
        store
            .report_task_blocked(
                &task.task_id,
                &run.run_id,
                "lease:blocked",
                "Should archived records count?",
                "The requested date range is ambiguous.",
            )
            .await
            .expect("blocked");
        let (_, resumed_run) = store
            .resume_task(
                &task.task_id,
                "human:local",
                "human:local",
                Some("Exclude archived records from the result."),
            )
            .await
            .expect("resumed");

        let submission = crate::TaskSubmissionRecord {
            submission_id: "submission:test".to_string(),
            task_id: task.task_id,
            executor_run_id: resumed_run.run_id,
            revision_index: 0,
            summary: "Done".to_string(),
            result_markdown: "Done".to_string(),
            criteria: Vec::new(),
            artifacts: Vec::new(),
            created_at: "now".to_string(),
        };
        let context = human_continuation_context_for_submission(&store, &submission)
            .await
            .expect("human context");

        assert_eq!(
            context,
            vec![TaskHumanContinuationContext {
                question: Some("Should archived records count?".to_string()),
                answer: "Exclude archived records from the result.".to_string(),
            }]
        );
    }
}
