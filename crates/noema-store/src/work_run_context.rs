//! Read-consistent, bounded execution context for a supervised Work run.

use noema_tasks::{
    AgentRunItemRecord, AgentRunRecord, ProjectRunContext, RunStatus, TaskGateRecord,
    TaskMessageKind, TaskMessageRecord, TaskRecord, WorkspaceRunContext,
};
use noema_workspaces::{ProjectRecord, WorkspaceRecord};
use rusqlite::{OptionalExtension, Row, Transaction, params};

use crate::{
    NoemaStore, StoreError,
    work_reads::history::decode_message,
    work_reads::rows::{
        decode_gate, load_active_gate, load_project, load_stage, load_task, load_workflow,
        load_workspace, validate_current_links,
    },
    work_run_context_records::{
        WORK_RUN_CONTEXT_MAX_GATES, WORK_RUN_CONTEXT_MAX_MESSAGES, WorkRunExecutionContext,
    },
    work_runs::rows::load_run_tx,
};

const MAX_CONTEXT_TEXT_BYTES: usize = 64 * 1024;
pub(super) const MAX_CONTEXT_PAYLOAD_BYTES: usize = 128 * 1024;

impl NoemaStore {
    /// Load one run envelope and its bounded, role-relevant durable evidence.
    ///
    /// All rows are read from one deferred SQLite transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when the run context rows are malformed, inconsistent,
    /// or exceed their bounded projection limits.
    pub async fn get_work_run_execution_context(
        &self,
        run_id: &str,
    ) -> Result<Option<WorkRunExecutionContext>, StoreError> {
        let run_id = run_id.trim().to_owned();
        if run_id.is_empty() {
            return Err(StoreError::Work(
                noema_tasks::WorkDomainError::InvalidInput {
                    field: "run_id",
                    message: "run id cannot be blank".to_string(),
                },
            ));
        }
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let context = load_work_run_execution_context_tx(&transaction, &run_id)?;
            transaction.commit()?;
            Ok(context)
        })
        .await
    }
}

pub(super) fn load_work_run_execution_context_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<Option<WorkRunExecutionContext>, StoreError> {
    let Some(run) = load_run_tx(transaction, run_id)? else {
        return Ok(None);
    };
    let task =
        load_task(transaction, &run.task_id)?.ok_or_else(|| StoreError::InvariantViolation {
            message: format!("run {} references missing task {}", run.run_id, run.task_id),
        })?;
    let source_runtime_environment =
        load_source_runtime_environment(transaction, &task.provenance)?;
    validate_run_task_fence(&run, &task)?;
    ensure_context_text(&task.title, "task.title")?;
    ensure_context_text(&task.description_markdown, "task.description_markdown")?;

    let workspace_row = load_workspace(transaction, &task.workspace_id)?;
    let project_row = task
        .project_id
        .as_ref()
        .map(|project_id| load_project(transaction, project_id))
        .transpose()?;
    if project_row
        .as_ref()
        .is_some_and(|project| project.workspace_id != task.workspace_id)
    {
        return Err(StoreError::InvariantViolation {
            message: format!("task {} project crosses workspace fence", task.task_id),
        });
    }
    let workflow = load_workflow(transaction, &task.workflow_id)?;
    let stage = load_stage(transaction, &task.stage_id)?;
    stage
        .belongs_to(&workflow.workflow_id)
        .map_err(StoreError::Work)?;

    let (workspace, project) = live_context(&workspace_row, project_row.as_ref())?;

    let active_gate = task
        .active_gate_id
        .as_ref()
        .map(|gate_id| load_active_gate(transaction, gate_id))
        .transpose()?;
    validate_current_links(&task, Some(&run), active_gate.as_ref())?;
    let relevant_gates = load_relevant_gates(transaction, &task, &run, active_gate.as_ref())?;
    let messages = load_relevant_messages(transaction, &task, &run, &relevant_gates)?;

    let lineage = Vec::new();
    Ok(Some(WorkRunExecutionContext {
        run,
        task,
        source_runtime_environment,
        workflow,
        stage,
        workspace,
        project,
        active_gate,
        relevant_gates,
        messages,
        lineage,
    }))
}

fn load_source_runtime_environment(
    transaction: &Transaction<'_>,
    provenance: &noema_tasks::TaskProvenance,
) -> Result<Option<String>, StoreError> {
    let (Some(conversation_id), Some(item_id)) = (
        provenance.conversation_id.as_deref(),
        provenance.item_id.as_deref(),
    ) else {
        return Ok(None);
    };
    transaction
        .query_row(
            r#"
            SELECT context.content_text
            FROM conversation_items AS source
            JOIN conversation_items AS context
              ON context.conversation_id = source.conversation_id
             AND context.sequence_index < source.sequence_index
            WHERE source.item_id = ?1
              AND source.conversation_id = ?2
              AND source.deleted_at IS NULL
              AND context.deleted_at IS NULL
              AND context.kind = 'model_context_update'
              AND context.status = 'completed'
              AND context.content_text IS NOT NULL
              AND json_extract(
                    context.payload_json,
                    '$.model_context_update.section_id'
                  ) = 'runtime.environment'
            ORDER BY context.sequence_index DESC
            LIMIT 1
            "#,
            params![item_id, conversation_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|value| bounded_text(value, "task.source_runtime_environment"))
        .transpose()
}

fn validate_run_task_fence(run: &AgentRunRecord, task: &TaskRecord) -> Result<(), StoreError> {
    if !matches!(run.status, RunStatus::Leased | RunStatus::Running) {
        return Err(StoreError::InvariantViolation {
            message: format!("run {} is not leased or running", run.run_id),
        });
    }
    if run.task_id != task.task_id
        || run.task_generation != task.generation
        || task.latest_run_id.as_deref() != Some(run.run_id.as_str())
    {
        return Err(StoreError::InvariantViolation {
            message: format!("run {} crosses task generation fence", run.run_id),
        });
    }
    Ok(())
}

fn live_context(
    workspace: &WorkspaceRecord,
    project: Option<&ProjectRecord>,
) -> Result<(WorkspaceRunContext, Option<ProjectRunContext>), StoreError> {
    let workspace_snapshot = WorkspaceRunContext {
        workspace_id: workspace.workspace_id.clone(),
        name: bounded_text(workspace.name.clone(), "workspace.name")?,
        description: bounded_text(workspace.description.clone(), "workspace.description")?,
    };
    let project_snapshot = project
        .map(|project| {
            Ok::<ProjectRunContext, StoreError>(ProjectRunContext {
                project_id: project.project_id.clone(),
                name: bounded_text(project.name.clone(), "project.name")?,
                description: bounded_text(project.description.clone(), "project.description")?,
                folder: project.folder.clone(),
            })
        })
        .transpose()?;
    Ok((workspace_snapshot, project_snapshot))
}

fn load_relevant_gates(
    transaction: &Transaction<'_>,
    task: &TaskRecord,
    run: &AgentRunRecord,
    active_gate: Option<&TaskGateRecord>,
) -> Result<Vec<TaskGateRecord>, StoreError> {
    let parent_run_id = run.parent_run_id.as_deref();
    let mut statement = transaction.prepare(
        "SELECT gate_id, task_id, task_generation, gate_kind, gate_state, recovery_reason, retry_run_kind, prompt_markdown, context_markdown, suggested_answers_json, opened_by_actor_id, originating_run_id, resolved_by_actor_id, resolution_message_id, opened_at, resolved_at FROM task_gates WHERE task_id = ?1 AND task_generation = ?2 AND (originating_run_id = ?3 OR originating_run_id = ?4 OR gate_id = ?5 OR resolution_message_id IN (SELECT message_id FROM task_messages WHERE consumed_by_run_id IN (?3, ?4)) OR gate_id IN (SELECT gate_id FROM task_messages WHERE task_id = ?1 AND task_generation = ?2 AND gate_id IS NOT NULL AND (consumed_by_run_id = ?3 OR consumed_by_run_id = ?4 OR consumed_by_run_id IS NULL))) ORDER BY opened_at, gate_id LIMIT ?6",
    )?;
    let rows = statement.query_map(
        params![
            task.task_id.as_str(),
            i64::try_from(task.generation).map_err(|_| StoreError::InvariantViolation {
                message: "task generation exceeds SQLite range".to_string()
            })?,
            run.run_id.as_str(),
            parent_run_id,
            active_gate.map(|gate| gate.gate_id.as_str()),
            (WORK_RUN_CONTEXT_MAX_GATES + 1) as i64,
        ],
        decode_gate,
    )?;
    let mut gates = rows.collect::<Result<Vec<_>, _>>()?;
    if gates.len() > WORK_RUN_CONTEXT_MAX_GATES {
        return Err(StoreError::InvariantViolation {
            message: format!("task {} exceeds bounded gate context", task.task_id),
        });
    }
    for gate in &gates {
        gate.validate().map_err(StoreError::Work)?;
    }
    if let Some(active_gate) = active_gate
        && !gates.iter().any(|gate| gate.gate_id == active_gate.gate_id)
    {
        gates.push(active_gate.clone());
        gates.sort_by(|left, right| {
            left.opened_at
                .cmp(&right.opened_at)
                .then_with(|| left.gate_id.as_str().cmp(right.gate_id.as_str()))
        });
    }
    Ok(gates)
}

fn load_relevant_messages(
    transaction: &Transaction<'_>,
    task: &TaskRecord,
    run: &AgentRunRecord,
    gates: &[TaskGateRecord],
) -> Result<Vec<TaskMessageRecord>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT message_id, task_id, task_generation, gate_id, message_kind, body_markdown, approval_decision, author_actor_id, consumed_by_run_id, consumed_at, created_at FROM task_messages WHERE task_id = ?1 AND task_generation = ?2 AND (consumed_by_run_id = ?3 OR consumed_by_run_id = ?4 OR consumed_by_run_id IS NULL) ORDER BY created_at, message_id LIMIT ?5",
    )?;
    let rows = statement.query_map(
        params![
            task.task_id.as_str(),
            i64::try_from(task.generation).map_err(|_| StoreError::InvariantViolation {
                message: "task generation exceeds SQLite range".to_string()
            })?,
            run.run_id.as_str(),
            run.parent_run_id.as_deref(),
            (WORK_RUN_CONTEXT_MAX_MESSAGES + 1) as i64,
        ],
        decode_message,
    )?;
    let messages = rows.collect::<Result<Vec<_>, _>>()?;
    if messages.len() > WORK_RUN_CONTEXT_MAX_MESSAGES {
        return Err(StoreError::InvariantViolation {
            message: format!("task {} exceeds bounded message context", task.task_id),
        });
    }
    for message in &messages {
        validate_message(message, task)?;
        if message
            .gate_id
            .as_ref()
            .is_some_and(|gate_id| !gates.iter().any(|gate| &gate.gate_id == gate_id))
        {
            return Err(StoreError::InvariantViolation {
                message: format!("message {} references unrelated gate", message.message_id),
            });
        }
    }
    Ok(messages)
}

pub(super) fn decode_run_item(row: &Row<'_>) -> rusqlite::Result<AgentRunItemRecord> {
    crate::work_runs::rows::decode_run_item(row, Some(MAX_CONTEXT_PAYLOAD_BYTES))
}

fn validate_message(message: &TaskMessageRecord, task: &TaskRecord) -> Result<(), StoreError> {
    if message.task_id != task.task_id
        || message.task_generation != task.generation
        || message.body_markdown.trim().is_empty()
    {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "message {} crosses task fence or is blank",
                message.message_id
            ),
        });
    }
    ensure_context_text(&message.body_markdown, "task_message.body_markdown")?;
    ensure_context_text(&message.author_actor_id, "task_message.author_actor_id")?;
    if message.kind != TaskMessageKind::HumanAnswer && message.approval_decision.is_some() {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "message {} has an approval decision on a non-answer",
                message.message_id
            ),
        });
    }
    Ok(())
}

fn ensure_context_text(value: &str, field: &'static str) -> Result<(), StoreError> {
    if value.len() > MAX_CONTEXT_TEXT_BYTES {
        return Err(StoreError::InvariantViolation {
            message: format!("{field} exceeds bounded context size"),
        });
    }
    Ok(())
}

fn bounded_text(value: String, field: &'static str) -> Result<String, StoreError> {
    ensure_context_text(&value, field)?;
    Ok(value)
}

#[cfg(test)]
mod source_runtime_environment_tests {
    use noema_tasks::{TaskProvenance, TaskSourceKind};

    use super::*;

    #[tokio::test]
    async fn task_source_keeps_the_last_runtime_environment_before_the_human_item() {
        let store = crate::tests::test_store().await;
        let environment = r"NOEMA_MODEL_CONTEXT_UPDATE\ncurrent_date: 2026-08-12";
        let provenance = TaskProvenance {
            source_kind: TaskSourceKind::ChatDelegate,
            conversation_id: Some("conversation:test".to_string()),
            turn_id: Some("turn:test".to_string()),
            item_id: Some("item:human".to_string()),
            source_tool_call_id: None,
            created_by_actor_id: "actor:agent:primary".to_string(),
        };

        let found = store
            .with_connection(move |connection| {
                connection.execute_batch(
                    r#"
                    INSERT INTO conversation_items
                      (item_id, conversation_id, sequence_index, kind, status,
                       author_actor_id, content_text, payload_json)
                    VALUES
                      ('item:old-environment', 'conversation:test', 1,
                       'model_context_update', 'completed', 'agent:primary',
                       'NOEMA_MODEL_CONTEXT_UPDATE\ncurrent_date: 2026-08-11',
                       '{"model_context_update":{"section_id":"runtime.environment"}}'),
                      ('item:environment', 'conversation:test', 2,
                       'model_context_update', 'completed', 'agent:primary',
                       'NOEMA_MODEL_CONTEXT_UPDATE\ncurrent_date: 2026-08-12',
                       '{"model_context_update":{"section_id":"runtime.environment"}}'),
                      ('item:human', 'conversation:test', 3, 'user_text',
                       'completed', 'human:local', 'Find tonight''s concert.', '{}'),
                      ('item:later-environment', 'conversation:test', 4,
                       'model_context_update', 'completed', 'agent:primary',
                       'NOEMA_MODEL_CONTEXT_UPDATE\ncurrent_date: 2026-08-13',
                       '{"model_context_update":{"section_id":"runtime.environment"}}');
                    "#,
                )?;
                let transaction = connection.transaction()?;
                load_source_runtime_environment(&transaction, &provenance)
            })
            .await
            .expect("source environment");

        assert_eq!(found.as_deref(), Some(environment));
    }
}
