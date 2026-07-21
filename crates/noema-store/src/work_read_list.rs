//! Bounded, batch-hydrated task connection reads.

use noema_tasks::WorkflowStageBehavior;
use rusqlite::{Transaction, params};

use super::{
    list_rows::{
        TaskPageRow, decode_task_page_row, load_gates, load_projects, load_reviews, load_runs,
    },
    rows::{derive_attention_actions, load_workspace, validate_current_links},
};
use crate::{
    NoemaStore, StoreError, WorkPageInfo, WorkTaskConnection, WorkTaskCursor, WorkTaskEdge,
    WorkTaskQuery, WorkTaskScope, WorkTaskSummary,
};

const TASK_COLUMNS: &str = "
    task.task_id,
    task.workspace_id,
    task.project_id,
    task.workflow_id,
    task.stage_id,
    task.title,
    task.description_markdown,
    task.authorization_context_json,
    task.source_kind,
    task.source_conversation_id,
    task.source_turn_id,
    task.source_item_id,
    task.source_tool_call_id,
    task.created_by_actor_id,
    task.generation,
    task.revision,
    task.current_contract_id,
    task.active_gate_id,
    task.latest_run_id,
    task.latest_submission_id,
    task.latest_review_id,
    task.completed_submission_id,
    task.queued_at,
    task.created_at,
    task.updated_at,
    task.completed_at,
    task.cancelled_at
";

const STAGE_COLUMNS: &str = "
    stage.stage_id,
    stage.workflow_id,
    stage.stable_key,
    stage.display_name,
    stage.ordinal,
    stage.system_behavior,
    stage.board_visible
";

impl NoemaStore {
    /// List strict task cards using a query-bound exclusive keyset cursor.
    /// Related current runs, gates, and reviews are hydrated in fixed batches.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for a mismatched cursor, SQLite failure, missing
    /// current pointer, or any noncanonical task-card row.
    pub async fn list_work_tasks(
        &self,
        query: WorkTaskQuery,
    ) -> Result<WorkTaskConnection, StoreError> {
        let prepared = PreparedQuery::new(query)?;
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            load_connection(&transaction, prepared)
        })
        .await
    }
}

pub(crate) struct PreparedQuery {
    workspace_id: String,
    project_id: Option<String>,
    stage_ids_json: String,
    stage_behaviors_json: String,
    text: Option<String>,
    attention_only: i64,
    scope: WorkTaskScope,
    first: usize,
    cursor_timestamp: Option<String>,
    cursor_task_id: Option<String>,
    query_hash: String,
}

impl PreparedQuery {
    pub(crate) fn new(mut query: WorkTaskQuery) -> Result<Self, StoreError> {
        query
            .stage_ids
            .sort_by(|left, right| left.as_str().cmp(right.as_str()));
        query.stage_ids.dedup();
        query
            .stage_behaviors
            .sort_by_key(|behavior| behavior.as_str());
        query.stage_behaviors.dedup();
        query.text = query
            .text
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty());
        let query_hash = task_query_hash(&query);
        let expected_terminal_cursor = query.scope == WorkTaskScope::Terminal;
        let (cursor_hash, cursor_timestamp, cursor_task_id) = match query.after {
            None => (None, None, None),
            Some(WorkTaskCursor::Active {
                query_hash,
                updated_at,
                task_id,
            }) if !expected_terminal_cursor => (
                Some(query_hash),
                Some(updated_at),
                Some(task_id.into_string()),
            ),
            Some(WorkTaskCursor::Terminal {
                query_hash,
                terminal_at,
                task_id,
            }) if expected_terminal_cursor => (
                Some(query_hash),
                Some(terminal_at),
                Some(task_id.into_string()),
            ),
            Some(_) => return Err(invalid_task_cursor()),
        };
        if cursor_hash.as_ref().is_some_and(|hash| hash != &query_hash) {
            return Err(invalid_task_cursor());
        }
        let stage_ids = query
            .stage_ids
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>();
        let stage_behaviors = query
            .stage_behaviors
            .iter()
            .map(|behavior| behavior.as_str())
            .collect::<Vec<_>>();
        Ok(Self {
            workspace_id: query.workspace_id.into_string(),
            project_id: query.project_id.map(|id| id.into_string()),
            stage_ids_json: serde_json::to_string(&stage_ids)?,
            stage_behaviors_json: serde_json::to_string(&stage_behaviors)?,
            text: query.text,
            attention_only: i64::from(query.attention_only),
            scope: query.scope,
            first: query.first.get(),
            cursor_timestamp,
            cursor_task_id,
            query_hash,
        })
    }
}

pub(crate) fn load_connection(
    transaction: &Transaction<'_>,
    query: PreparedQuery,
) -> Result<WorkTaskConnection, StoreError> {
    let limit = i64::try_from(query.first + 1).map_err(|error| StoreError::InvariantViolation {
        message: format!("task page size could not be represented in SQLite: {error}"),
    })?;
    let scope_predicate = match query.scope {
        WorkTaskScope::Active => {
            "stage.system_behavior NOT IN ('terminal_success', 'terminal_cancelled')"
        }
        WorkTaskScope::Terminal => {
            "stage.system_behavior IN ('terminal_success', 'terminal_cancelled')"
        }
        WorkTaskScope::All => "1 = 1",
    };
    let (cursor_predicate, order_by) = if query.scope == WorkTaskScope::Terminal {
        (
            "(?7 IS NULL OR COALESCE(task.completed_at, task.cancelled_at) < ?7
               OR (COALESCE(task.completed_at, task.cancelled_at) = ?7 AND task.task_id < ?8))",
            "COALESCE(task.completed_at, task.cancelled_at) DESC, task.task_id DESC",
        )
    } else {
        (
            "(?7 IS NULL OR task.updated_at < ?7
               OR (task.updated_at = ?7 AND task.task_id < ?8))",
            "task.updated_at DESC, task.task_id DESC",
        )
    };
    let sql = format!(
        "SELECT {TASK_COLUMNS}, {STAGE_COLUMNS}
         FROM tasks task
         JOIN workflow_stages stage
           ON stage.workflow_id = task.workflow_id AND stage.stage_id = task.stage_id
         WHERE task.workspace_id = ?1
           AND (?2 IS NULL OR task.project_id = ?2)
           AND (json_array_length(?3) = 0
                OR stage.stage_id IN (SELECT value FROM json_each(?3)))
           AND (json_array_length(?4) = 0
                OR stage.system_behavior IN (SELECT value FROM json_each(?4)))
           AND (?5 IS NULL
                OR instr(lower(task.title), ?5) > 0
                OR instr(lower(task.description_markdown), ?5) > 0)
           AND (?6 = 0 OR (
                (stage.system_behavior = 'human_gate' AND EXISTS (
                    SELECT 1 FROM task_gates gate
                    WHERE gate.gate_id = task.active_gate_id
                      AND gate.task_id = task.task_id
                      AND gate.task_generation = task.generation
                      AND gate.gate_state = 'open'
                ))
           ))
           AND {scope_predicate}
           AND {cursor_predicate}
         ORDER BY {order_by}
         LIMIT ?9"
    );
    let mut statement = transaction.prepare(&sql)?;
    let rows = statement.query_map(
        params![
            query.workspace_id,
            query.project_id,
            query.stage_ids_json,
            query.stage_behaviors_json,
            query.text,
            query.attention_only,
            query.cursor_timestamp,
            query.cursor_task_id,
            limit,
        ],
        decode_task_page_row,
    )?;
    let mut page_rows = rows.collect::<Result<Vec<_>, _>>()?;
    let has_next_page = page_rows.len() > query.first;
    page_rows.truncate(query.first);

    let run_ids = pointer_ids(&page_rows, |row| row.task.latest_run_id.as_deref());
    let gate_ids = pointer_ids(&page_rows, |row| {
        row.task.active_gate_id.as_ref().map(|id| id.as_str())
    });
    let review_ids = pointer_ids(&page_rows, |row| row.task.latest_review_id.as_deref());
    let project_ids = pointer_ids(&page_rows, |row| {
        row.task.project_id.as_ref().map(|id| id.as_str())
    });
    let workspace = page_rows
        .first()
        .map(|row| load_workspace(transaction, &row.task.workspace_id))
        .transpose()?;
    let runs = load_runs(transaction, &run_ids)?;
    let gates = load_gates(transaction, &gate_ids)?;
    let reviews = load_reviews(transaction, &review_ids)?;
    let projects = load_projects(transaction, &project_ids)?;

    let edges = page_rows
        .into_iter()
        .map(|row| {
            task_edge(
                row,
                workspace.as_ref(),
                &projects,
                &runs,
                &gates,
                &reviews,
                &query,
            )
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let end_cursor = edges.last().map(|edge| edge.cursor.clone());
    Ok(WorkTaskConnection {
        edges,
        page_info: WorkPageInfo {
            end_cursor,
            has_next_page,
        },
    })
}

fn task_edge(
    row: TaskPageRow,
    workspace: Option<&noema_workspaces::WorkspaceRecord>,
    projects: &std::collections::HashMap<String, noema_workspaces::ProjectRecord>,
    runs: &std::collections::HashMap<String, noema_tasks::AgentRunRecord>,
    gates: &std::collections::HashMap<String, noema_tasks::TaskGateRecord>,
    reviews: &std::collections::HashMap<String, noema_tasks::TaskReviewRecord>,
    query: &PreparedQuery,
) -> Result<WorkTaskEdge, StoreError> {
    let workspace = workspace
        .filter(|workspace| workspace.workspace_id == row.task.workspace_id)
        .cloned()
        .ok_or_else(|| missing_pointer("workspace", row.task.workspace_id.as_str()))?;
    let project = row
        .task
        .project_id
        .as_ref()
        .map(|id| {
            projects
                .get(id.as_str())
                .filter(|project| project.workspace_id == row.task.workspace_id)
                .cloned()
                .ok_or_else(|| missing_pointer("project", id.as_str()))
        })
        .transpose()?;
    let current_run = row
        .task
        .latest_run_id
        .as_ref()
        .map(|id| {
            runs.get(id)
                .cloned()
                .ok_or_else(|| missing_pointer("run", id))
        })
        .transpose()?
        .filter(|run| {
            matches!(
                run.status,
                noema_tasks::RunStatus::Queued
                    | noema_tasks::RunStatus::Leased
                    | noema_tasks::RunStatus::Running
            )
        });
    let active_gate = row
        .task
        .active_gate_id
        .as_ref()
        .map(|id| {
            gates
                .get(id.as_str())
                .cloned()
                .ok_or_else(|| missing_pointer("gate", id.as_str()))
        })
        .transpose()?;
    let latest_review = row
        .task
        .latest_review_id
        .as_ref()
        .map(|id| {
            reviews
                .get(id)
                .cloned()
                .ok_or_else(|| missing_pointer("review", id))
        })
        .transpose()?;
    validate_summary_links(
        &row.task,
        &row.stage,
        current_run.as_ref(),
        active_gate.as_ref(),
        latest_review.as_ref(),
    )?;
    let (attention, valid_actions) =
        derive_attention_actions(row.stage.system_behavior, active_gate.as_ref());
    let cursor = if query.scope == WorkTaskScope::Terminal {
        WorkTaskCursor::terminal(
            query.query_hash.clone(),
            terminal_timestamp(&row.task, row.stage.system_behavior)?.to_string(),
            row.task.task_id.clone(),
        )
        .map_err(|_| StoreError::InvariantViolation {
            message: "task terminal cursor timestamp is not canonical".to_string(),
        })?
        .encode()
    } else {
        WorkTaskCursor::active(
            query.query_hash.clone(),
            row.task.updated_at.clone(),
            row.task.task_id.clone(),
        )
        .map_err(|_| StoreError::InvariantViolation {
            message: "task cursor timestamp is not canonical".to_string(),
        })?
        .encode()
    };
    Ok(WorkTaskEdge {
        cursor,
        node: WorkTaskSummary {
            task: row.task,
            workspace,
            project,
            stage: row.stage,
            current_run,
            active_gate,
            latest_review,
            attention,
            valid_actions,
        },
    })
}

fn validate_summary_links(
    task: &noema_tasks::TaskRecord,
    stage: &noema_tasks::WorkflowStage,
    run: Option<&noema_tasks::AgentRunRecord>,
    gate: Option<&noema_tasks::TaskGateRecord>,
    review: Option<&noema_tasks::TaskReviewRecord>,
) -> Result<(), StoreError> {
    stage
        .belongs_to(&task.workflow_id)
        .map_err(StoreError::Work)?;
    validate_current_links(task, run, gate)?;
    let contract_id = task.current_contract_id.as_ref();
    if run.is_some_and(|run| run.contract_id.as_ref() != contract_id)
        || gate.is_some_and(|gate| gate.contract_id.as_ref() != contract_id)
        || review.is_some_and(|review| {
            review.task_id != task.task_id || Some(&review.contract_id) != contract_id
        })
    {
        return Err(StoreError::InvariantViolation {
            message: format!("task {} card crosses a current contract link", task.task_id),
        });
    }
    validate_lifecycle(task, stage.system_behavior)
}

fn validate_lifecycle(
    task: &noema_tasks::TaskRecord,
    behavior: WorkflowStageBehavior,
) -> Result<(), StoreError> {
    let valid = match behavior {
        WorkflowStageBehavior::TerminalSuccess => {
            task.completed_at.is_some()
                && task.cancelled_at.is_none()
                && task.completed_submission_id.is_some()
        }
        WorkflowStageBehavior::TerminalCancelled => {
            task.completed_at.is_none() && task.cancelled_at.is_some()
        }
        _ => task.completed_at.is_none() && task.cancelled_at.is_none(),
    };
    if valid {
        Ok(())
    } else {
        Err(StoreError::InvariantViolation {
            message: format!("task {} lifecycle does not match its stage", task.task_id),
        })
    }
}

fn terminal_timestamp(
    task: &noema_tasks::TaskRecord,
    behavior: WorkflowStageBehavior,
) -> Result<&str, StoreError> {
    match behavior {
        WorkflowStageBehavior::TerminalSuccess => task.completed_at.as_deref(),
        WorkflowStageBehavior::TerminalCancelled => task.cancelled_at.as_deref(),
        _ => None,
    }
    .ok_or_else(|| StoreError::InvariantViolation {
        message: format!("task {} has no terminal cursor timestamp", task.task_id),
    })
}

fn pointer_ids<'a>(
    rows: &'a [TaskPageRow],
    pointer: impl Fn(&'a TaskPageRow) -> Option<&'a str>,
) -> Vec<String> {
    let mut ids = rows
        .iter()
        .filter_map(pointer)
        .map(str::to_string)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn task_query_hash(query: &WorkTaskQuery) -> String {
    let stages = query
        .stage_ids
        .iter()
        .map(|id| id.as_str())
        .collect::<Vec<_>>()
        .join("\u{1f}");
    let behaviors = query
        .stage_behaviors
        .iter()
        .map(|behavior| behavior.as_str())
        .collect::<Vec<_>>()
        .join("\u{1f}");
    let canonical = format!(
        "work-task-query:v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        query.workspace_id.as_str(),
        query.project_id.as_ref().map_or("", |id| id.as_str()),
        stages,
        behaviors,
        query.text.as_deref().unwrap_or(""),
        u8::from(query.attention_only),
        query.scope.as_str(),
    );
    crate::work_row::sha256_hex(canonical.as_bytes())
}

fn invalid_task_cursor() -> StoreError {
    StoreError::Work(noema_tasks::WorkDomainError::InvalidInput {
        field: "work_task.cursor",
        message: "invalid_cursor".to_string(),
    })
}

fn missing_pointer(kind: &str, id: &str) -> StoreError {
    StoreError::InvariantViolation {
        message: format!("task references missing {kind}: {id}"),
    }
}
