//! Submission-fenced reads of exact executor-run evidence.

use noema_capabilities::ToolSpec;
use noema_store::{
    NoemaStore, WorkPageSize, WorkRunItemCursor, WorkRunItemOwnerScope, WorkRunItemQuery,
};
use noema_tasks::RunKind;
use serde::Deserialize;
use serde_json::{Value, json};

pub(crate) const TASK_READ_SUBMISSION_EVIDENCE_TOOL: &str = "task.read_submission_evidence";
const DEFAULT_PAGE_SIZE: u32 = 12;
const MAX_PAGE_SIZE: u32 = 25;
const CONTENT_PREVIEW_CHARS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskSubmissionEvidenceContext {
    pub(crate) task_id: String,
    pub(crate) run_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadSubmissionEvidenceArguments {
    #[serde(default)]
    item_id: Option<String>,
    #[serde(default)]
    before: Option<String>,
    #[serde(default = "default_page_size")]
    first: u32,
}

const fn default_page_size() -> u32 {
    DEFAULT_PAGE_SIZE
}

#[must_use]
pub(crate) fn is_task_read_submission_evidence_tool(name: &str) -> bool {
    name == TASK_READ_SUBMISSION_EVIDENCE_TOOL
}

pub(crate) fn task_read_submission_evidence_tool_spec()
-> Result<ToolSpec, noema_capabilities::ToolContractError> {
    ToolSpec::new(
        TASK_READ_SUBMISSION_EVIDENCE_TOOL,
        "List bounded metadata for the submitted Executor run. Read one exact saved item by item_id. The current review or correction review fences each read. A page keeps its items in chronological order.",
        json!({
            "type": "object",
            "properties": {
                "item_id": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 256,
                    "description": "Exact item ID returned by a prior list call. Omit to list item metadata."
                },
                "before": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 2048,
                    "description": "Opaque next_before cursor returned by a prior list call. Omit for the newest page."
                },
                "first": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_PAGE_SIZE,
                    "default": DEFAULT_PAGE_SIZE,
                    "description": "Maximum metadata rows returned by a list call."
                }
            },
            "required": [],
            "additionalProperties": false
        }),
    )
}

pub(crate) async fn execute_task_read_submission_evidence(
    store: &NoemaStore,
    context: &TaskSubmissionEvidenceContext,
    payload: &Value,
) -> Result<Value, String> {
    let arguments: ReadSubmissionEvidenceArguments = serde_json::from_value(payload.clone())
        .map_err(|error| format!("invalid submission evidence arguments: {error}"))?;
    if arguments.first == 0 || arguments.first > MAX_PAGE_SIZE {
        return Err("submission evidence page size is invalid".to_string());
    }
    if arguments.item_id.is_some()
        && (arguments.before.is_some() || arguments.first != DEFAULT_PAGE_SIZE)
    {
        return Err("item_id cannot be combined with list arguments".to_string());
    }
    let envelope = store
        .get_work_run_execution_context(&context.run_id)
        .await
        .map_err(|_| "submission evidence context is unavailable".to_string())?
        .filter(|envelope| {
            if envelope.task.task_id.as_str() != context.task_id {
                return false;
            }
            let Some(submission) = envelope.latest_submission.as_ref() else {
                return false;
            };
            match envelope.run.run_kind {
                RunKind::Reviewer => {
                    envelope.run.triggering_submission_id.as_deref()
                        == Some(submission.submission_id.as_str())
                }
                RunKind::Executor => {
                    envelope.run.review_round > 1
                        && envelope.run.triggering_review_id.as_deref()
                            == envelope
                                .latest_review
                                .as_ref()
                                .map(|review| review.review_id.as_str())
                        && envelope.latest_review.as_ref().is_some_and(|review| {
                            review.reviewed_submission_id == submission.submission_id
                        })
                }
                RunKind::Planner => false,
            }
        })
        .ok_or_else(|| "submission evidence context is unavailable".to_string())?;
    let submission = envelope
        .latest_submission
        .as_ref()
        .ok_or_else(|| "submission evidence is unavailable".to_string())?;
    let owner = WorkRunItemOwnerScope {
        workspace_id: envelope.task.workspace_id.clone(),
        task_id: Some(envelope.task.task_id.clone()),
    };
    if let Some(item_id) = arguments.item_id {
        let item = store
            .read_work_run_item(owner, &submission.executor_run_id, &item_id)
            .await
            .map_err(|_| "submission evidence item is unavailable".to_string())?
            .ok_or_else(|| "submission evidence item is unavailable".to_string())?;
        return Ok(json!({
            "submission_id": submission.submission_id,
            "executor_run_id": submission.executor_run_id,
            "item": item,
        }));
    }
    let before = arguments
        .before
        .as_deref()
        .map(WorkRunItemCursor::decode)
        .transpose()
        .map_err(|_| "submission evidence cursor is invalid".to_string())?;
    let first = WorkPageSize::new(arguments.first)
        .map_err(|_| "submission evidence page size is invalid".to_string())?;
    let page = store
        .list_work_run_items(WorkRunItemQuery {
            owner,
            run_id: submission.executor_run_id.clone(),
            first,
            before,
        })
        .await
        .map_err(|_| "submission evidence is unavailable".to_string())?;
    let items = page
        .edges
        .into_iter()
        .map(|edge| {
            let item = edge.node;
            let (content_preview, content_truncated) = item.content_text.as_deref().map_or_else(
                || (None, false),
                |content| {
                    let truncated = content.chars().count() > CONTENT_PREVIEW_CHARS;
                    let preview = content
                        .chars()
                        .take(CONTENT_PREVIEW_CHARS)
                        .collect::<String>();
                    (Some(preview), truncated)
                },
            );
            json!({
                "item_id": item.item_id,
                "sequence_index": item.sequence_index,
                "round_index": item.round_index,
                "kind": item.kind,
                "status": item.status,
                "correlation_id": item.correlation_id,
                "parent_item_id": item.parent_item_id,
                "content_preview": content_preview,
                "content_truncated": content_truncated,
                "payload_bytes": serde_json::to_vec(&item.payload).map_or(0, |bytes| bytes.len()),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "submission_id": submission.submission_id,
        "executor_run_id": submission.executor_run_id,
        "items": items,
        "next_before": page.page_info.end_cursor,
        "has_more": page.page_info.has_next_page,
    }))
}

#[cfg(test)]
#[path = "task_submission_evidence_tool/tests.rs"]
mod tests;
