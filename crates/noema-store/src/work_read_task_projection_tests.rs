use noema_tasks::{RunKind, TaskId, WorkflowStageBehavior};
use noema_workspaces::WorkspaceId;
use rusqlite::{Connection, params};

use crate::{
    NoemaStore, StoreError, WorkPageSize, WorkTaskAttention, WorkTaskCursor, WorkTaskQuery,
    WorkTaskScope, WorkTaskValidAction, test_support::open_ephemeral_store,
};

fn task_query(scope: WorkTaskScope, first: u32, after: Option<WorkTaskCursor>) -> WorkTaskQuery {
    WorkTaskQuery {
        workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
        project_id: None,
        stage_ids: Vec::new(),
        stage_behaviors: Vec::new(),
        text: None,
        attention_only: false,
        scope,
        first: WorkPageSize::new(first).expect("page size"),
        after,
    }
}

fn insert_task(
    connection: &Connection,
    id: &str,
    stage: &str,
    title: &str,
    updated_at: &str,
    completed_at: Option<&str>,
    cancelled_at: Option<&str>,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO tasks (
            task_id, workspace_id, workflow_id, stage_id, title,
            description_markdown, source_kind, created_by_actor_id,
            accepted_submission_id, created_at, updated_at, completed_at, cancelled_at
         ) VALUES (
            ?1, 'workspace:personal', 'workflow:personal:default', ?2, ?3,
            'Detailed searchable body', 'system', 'actor:system',
            ?4, ?5, ?5, ?6, ?7
         )",
        params![
            id,
            stage,
            title,
            completed_at.map(|_| "submission:accepted"),
            updated_at,
            completed_at,
            cancelled_at,
        ],
    )?;
    Ok(())
}

pub(super) async fn seed_task_cards(store: &NoemaStore) {
    store
        .with_connection(|connection| {
            insert_task(
                connection,
                "task:a",
                "stage:personal:inbox",
                "Alpha",
                "2026-02-03T00:00:00.000Z",
                None,
                None,
            )?;
            insert_task(
                connection,
                "task:b",
                "stage:personal:doing",
                "Beta",
                "2026-02-03T00:00:00.000Z",
                None,
                None,
            )?;
            insert_task(
                connection,
                "task:c",
                "stage:personal:queue",
                "Gamma",
                "2026-02-02T00:00:00.000Z",
                None,
                None,
            )?;
            insert_task(
                connection,
                "task:waiting",
                "stage:personal:waiting",
                "Waiting",
                "2026-02-01T00:00:00.000Z",
                None,
                None,
            )?;
            connection.execute(
                "INSERT INTO task_gates (
                    gate_id, task_id, task_generation, gate_kind, gate_state,
                    prompt_markdown, opened_by_actor_id
                 ) VALUES (
                    'gate:waiting', 'task:waiting', 1, 'clarification', 'open',
                    'Which option?', 'actor:system'
                 )",
                [],
            )?;
            connection.execute(
                "UPDATE tasks SET active_gate_id = 'gate:waiting'
                 WHERE task_id = 'task:waiting'",
                [],
            )?;
            insert_task(
                connection,
                "task:completed",
                "stage:personal:completed",
                "Completed",
                "2026-02-05T00:00:00.000Z",
                Some("2026-02-04T00:00:00.000Z"),
                None,
            )?;
            insert_task(
                connection,
                "task:cancelled",
                "stage:personal:cancelled",
                "Cancelled",
                "2026-02-06T00:00:00.000Z",
                None,
                Some("2026-02-05T00:00:00.000Z"),
            )?;
            Ok(())
        })
        .await
        .expect("seed task cards");
}

#[tokio::test]
async fn task_connection_batches_cards_and_uses_scope_specific_keysets() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_task_cards(&store).await;

    let first = store
        .list_work_tasks(task_query(WorkTaskScope::Active, 2, None))
        .await
        .expect("first active page");
    assert_eq!(
        first
            .edges
            .iter()
            .map(|edge| edge.node.task.task_id.as_str())
            .collect::<Vec<_>>(),
        ["task:b", "task:a"]
    );
    assert!(first.edges.iter().all(|edge| {
        edge.node.workspace.workspace_id == edge.node.task.workspace_id
            && edge.node.workspace.name == "Personal"
            && edge.node.project.is_none()
    }));
    assert!(first.page_info.has_next_page);
    let after = WorkTaskCursor::decode(
        first
            .page_info
            .end_cursor
            .as_deref()
            .expect("active cursor"),
    )
    .expect("decode active cursor");
    let second = store
        .list_work_tasks(task_query(WorkTaskScope::Active, 10, Some(after)))
        .await
        .expect("second active page");
    assert_eq!(
        second
            .edges
            .iter()
            .map(|edge| edge.node.task.task_id.as_str())
            .collect::<Vec<_>>(),
        ["task:c", "task:waiting"]
    );
    assert_eq!(
        second.edges[1].node.attention,
        Some(WorkTaskAttention::Clarification)
    );

    let terminal = store
        .list_work_tasks(task_query(WorkTaskScope::Terminal, 10, None))
        .await
        .expect("terminal page");
    assert_eq!(
        terminal
            .edges
            .iter()
            .map(|edge| edge.node.task.task_id.as_str())
            .collect::<Vec<_>>(),
        ["task:cancelled", "task:completed"]
    );
    assert!(matches!(
        WorkTaskCursor::decode(&terminal.edges[0].cursor).expect("terminal cursor"),
        WorkTaskCursor::Terminal { .. }
    ));
}

#[tokio::test]
async fn task_connection_store_operations_stay_fixed_as_card_count_grows() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_task_cards(&store).await;
    store
        .with_connection(|connection| {
            for index in 0..128 {
                let task_id = format!("task:bulk-{index:03}");
                insert_task(
                    connection,
                    &task_id,
                    "stage:personal:inbox",
                    &format!("Bulk task {index}"),
                    "2026-02-07T00:00:00.000Z",
                    None,
                    None,
                )?;
            }
            Ok(())
        })
        .await
        .expect("seed bulk task cards");

    for first in [1, 100] {
        store.reset_operation_count_for_tests();
        let page = store
            .list_work_tasks(task_query(WorkTaskScope::Active, first, None))
            .await
            .expect("read task cards");
        assert_eq!(page.edges.len(), first as usize);
        assert_eq!(
            store.operation_count_for_tests(),
            1,
            "card hydration must use one Store operation for page size {first}"
        );
    }
}

#[tokio::test]
async fn task_connection_fails_closed_on_missing_or_malformed_placement_rows() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_task_cards(&store).await;
    store
        .with_connection(|connection| {
            connection.execute_batch("PRAGMA foreign_keys = OFF")?;
            connection.execute(
                "UPDATE tasks SET project_id = 'project:missing' WHERE task_id = 'task:a'",
                [],
            )?;
            connection.execute_batch("PRAGMA foreign_keys = ON")?;
            Ok(())
        })
        .await
        .expect("break project pointer");
    let missing = store
        .list_work_tasks(task_query(WorkTaskScope::Active, 10, None))
        .await
        .expect_err("missing project must fail closed");
    assert!(matches!(
        missing,
        StoreError::InvariantViolation { ref message }
            if message.contains("missing project")
    ));

    store
        .with_connection(|connection| {
            connection.execute_batch("PRAGMA ignore_check_constraints = ON")?;
            connection.execute(
                "UPDATE tasks SET project_id = NULL WHERE task_id = 'task:a'",
                [],
            )?;
            connection.execute(
                "UPDATE workspaces SET name = '' WHERE workspace_id = 'workspace:personal'",
                [],
            )?;
            connection.execute_batch("PRAGMA ignore_check_constraints = OFF")?;
            Ok(())
        })
        .await
        .expect("corrupt workspace");
    assert!(
        store
            .list_work_tasks(task_query(WorkTaskScope::Active, 10, None))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn task_connection_applies_attention_search_and_cursor_query_binding() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_task_cards(&store).await;

    let mut attention_query = task_query(WorkTaskScope::Active, 10, None);
    attention_query.attention_only = true;
    let attention = store
        .list_work_tasks(attention_query)
        .await
        .expect("attention cards");
    assert_eq!(attention.edges.len(), 1);
    assert_eq!(
        attention.edges[0].node.task.task_id.as_str(),
        "task:waiting"
    );
    assert_eq!(
        attention.edges[0].node.valid_actions,
        [WorkTaskValidAction::Answer, WorkTaskValidAction::Cancel]
    );

    let first = store
        .list_work_tasks(task_query(WorkTaskScope::Active, 1, None))
        .await
        .expect("cursor source");
    let cursor = WorkTaskCursor::decode(&first.edges[0].cursor).expect("cursor");
    let mut changed = task_query(WorkTaskScope::Active, 1, Some(cursor));
    changed.text = Some("alpha".to_string());
    assert!(matches!(
        store.list_work_tasks(changed).await,
        Err(StoreError::Work(
            noema_tasks::WorkDomainError::InvalidInput {
                field: "work_task.cursor",
                ..
            }
        ))
    ));
}

fn insert_contract_and_executor(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "INSERT INTO task_execution_contracts (
            contract_id, task_id, version, task_generation, origin,
            request_markdown, complexity,
            executor_provider_kind, executor_provider_account_id,
            executor_provider_instance_key, executor_selection_mode, executor_model_profile,
            reviewer_provider_kind, reviewer_provider_account_id,
            reviewer_provider_instance_key, reviewer_selection_mode, reviewer_model_profile,
            max_provider_continuations, max_tool_calls, max_active_minutes,
            progress_audit_interval, max_automatic_retries, max_review_rounds,
            workspace_id_snapshot, workspace_name_snapshot, workspace_description_snapshot,
            created_by_actor_id
         ) VALUES (
            'contract:evidence', 'task:evidence', 1, 1, 'delegated',
            'Complete the task', 'simple',
            'codex', 'provider_account:codex:default', 'provider_account:codex:default',
            'explicit_profile', 'gpt-5.6-luna',
            'codex', 'provider_account:codex:default', 'provider_account:codex:default',
            'explicit_profile', 'gpt-5.6-luna',
            80, 400, 120, 20, 3, 3,
            'workspace:personal', 'Personal', '', 'actor:system'
         );
         INSERT INTO task_contract_criteria (
            criterion_id, contract_id, ordinal, description
         ) VALUES ('criterion:evidence', 'contract:evidence', 1, 'Result is complete');
         INSERT INTO agent_runs (
            run_id, task_id, task_generation, contract_id, run_kind, agent_id,
            attempt_index, review_round, provider_kind, provider_account_id,
            provider_instance_key, selection_mode, model_profile,
            max_provider_continuations, max_tool_calls, max_active_minutes,
            progress_audit_interval, max_automatic_retries, max_review_rounds, status
         ) VALUES (
            'run:executor', 'task:evidence', 1, 'contract:evidence', 'executor',
            'agent:task-executor', 0, 1, 'codex', 'provider_account:codex:default',
            'provider_account:codex:default', 'explicit_profile', 'gpt-5.6-luna',
            80, 400, 120, 20, 3, 3, 'completed'
         );
         INSERT INTO task_submissions (
            submission_id, task_id, contract_id, executor_run_id, review_round,
            summary, result_markdown
         ) VALUES (
            'submission:evidence', 'task:evidence', 'contract:evidence',
            'run:executor', 1, 'Finished', 'Complete result'
         );
         INSERT INTO task_submission_criteria (
            submission_id, criterion_id, evidence_markdown
         ) VALUES ('submission:evidence', 'criterion:evidence', 'Complete evidence');",
    )?;
    Ok(())
}

pub(super) async fn seed_evidence_task(store: &NoemaStore, with_review: bool) {
    store
        .with_connection(move |connection| {
            insert_task(
                connection,
                "task:evidence",
                if with_review {
                    "stage:personal:review"
                } else {
                    "stage:personal:doing"
                },
                "Evidence",
                "2026-03-01T00:00:00.000Z",
                None,
                None,
            )?;
            insert_contract_and_executor(connection)?;
            if with_review {
                connection.execute_batch(
                    "INSERT INTO agent_runs (
                        run_id, task_id, task_generation, contract_id, run_kind, agent_id,
                        attempt_index, review_round, triggering_submission_id,
                        provider_kind, provider_account_id, provider_instance_key,
                        selection_mode, model_profile, max_provider_continuations,
                        max_tool_calls, max_active_minutes, progress_audit_interval,
                        max_automatic_retries, max_review_rounds, status
                     ) VALUES (
                        'run:reviewer', 'task:evidence', 1, 'contract:evidence', 'reviewer',
                        'agent:task-reviewer', 0, 1, 'submission:evidence',
                        'codex', 'provider_account:codex:default',
                        'provider_account:codex:default', 'explicit_profile', 'gpt-5.6-luna',
                        80, 400, 120, 20, 3, 3, 'completed'
                     );
                     INSERT INTO task_reviews (
                        review_id, task_id, contract_id, reviewer_run_id,
                        reviewed_submission_id, review_attempt_index,
                        overall_verdict, overall_feedback
                     ) VALUES (
                        'review:evidence', 'task:evidence', 'contract:evidence',
                        'run:reviewer', 'submission:evidence', 1, 'approve', 'Approved'
                     );
                     INSERT INTO task_review_criteria (
                        review_id, criterion_id, outcome
                     ) VALUES ('review:evidence', 'criterion:evidence', 'pass');",
                )?;
            }
            connection.execute(
                "UPDATE tasks SET current_contract_id = 'contract:evidence',
                    latest_run_id = ?2, latest_submission_id = 'submission:evidence',
                    latest_review_id = ?3
                 WHERE task_id = ?1",
                params![
                    "task:evidence",
                    if with_review {
                        "run:reviewer"
                    } else {
                        "run:executor"
                    },
                    with_review.then_some("review:evidence"),
                ],
            )?;
            Ok(())
        })
        .await
        .expect("seed evidence task");
}

#[tokio::test]
async fn task_detail_hydrates_current_contract_submission_and_review() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_evidence_task(&store, true).await;
    let detail = store
        .get_work_task(&TaskId::new("task:evidence").expect("task id"))
        .await
        .expect("detail read")
        .expect("task detail");
    assert_eq!(detail.current_contract.expect("contract").criteria.len(), 1);
    assert_eq!(
        detail.latest_submission.expect("submission").submission_id,
        "submission:evidence"
    );
    assert_eq!(
        detail.latest_review.expect("review").overall_verdict,
        noema_tasks::TaskReviewVerdict::Approve
    );
    assert_eq!(
        detail.attention,
        Some(WorkTaskAttention::ReadyForAcceptance)
    );
    assert_eq!(
        detail.valid_actions,
        [
            WorkTaskValidAction::Accept,
            WorkTaskValidAction::RequestChanges,
            WorkTaskValidAction::Cancel,
        ]
    );
}

#[tokio::test]
async fn reconciliation_detects_submission_waiting_for_reviewer_after_completed_executor() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_evidence_task(&store, false).await;
    let envelope = store
        .load_work_reconciliation_snapshot(&TaskId::new("task:evidence").expect("task id"))
        .await
        .expect("snapshot read")
        .expect("snapshot");
    assert!(envelope.snapshot.has_current_contract);
    assert!(envelope.snapshot.submission_waiting_for_review);
    assert!(!envelope.snapshot.has_runnable_run);
    assert_eq!(
        envelope.snapshot.stage_behavior,
        WorkflowStageBehavior::Active
    );
}

#[tokio::test]
async fn reconciliation_recovers_resume_role_from_resolved_gate_lineage() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_evidence_task(&store, false).await;
    store
        .with_connection(|connection| {
            connection.execute_batch(
                "INSERT INTO task_gates (
                    gate_id, task_id, task_generation, contract_id, gate_kind, gate_state,
                    prompt_markdown, opened_by_actor_id, originating_run_id
                 ) VALUES (
                    'gate:resolved', 'task:evidence', 1, 'contract:evidence',
                    'clarification', 'open', 'Clarify?', 'actor:system', 'run:executor'
                 );
                 INSERT INTO task_messages (
                    message_id, task_id, task_generation, contract_id, gate_id,
                    message_kind, body_markdown, author_actor_id
                 ) VALUES (
                    'task_message:resolved', 'task:evidence', 1, 'contract:evidence',
                    'gate:resolved', 'human_answer', 'Answer', 'actor:human'
                 );
                 UPDATE task_gates SET gate_state = 'resolved',
                    resolved_by_actor_id = 'actor:human',
                    resolution_message_id = 'task_message:resolved',
                    resolved_at = '2026-03-02T00:00:00.000Z'
                 WHERE gate_id = 'gate:resolved';
                 UPDATE tasks SET stage_id = 'stage:personal:waiting'
                 WHERE task_id = 'task:evidence';",
            )?;
            Ok(())
        })
        .await
        .expect("seed resolved gate");
    let envelope = store
        .load_work_reconciliation_snapshot(&TaskId::new("task:evidence").expect("task id"))
        .await
        .expect("snapshot read")
        .expect("snapshot");
    assert_eq!(
        envelope.snapshot.resolved_gate_resume_run_kind,
        Some(RunKind::Executor)
    );
}

#[tokio::test]
async fn reconciliation_uses_submission_round_for_review_exhaustion() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_evidence_task(&store, true).await;
    store
        .with_connection(|connection| {
            connection.execute_batch(
                "UPDATE task_submissions SET review_round = 3
                 WHERE submission_id = 'submission:evidence';
                 UPDATE agent_runs SET review_round = 3
                 WHERE run_id IN ('run:executor', 'run:reviewer');
                 UPDATE task_reviews SET overall_verdict = 'request_changes',
                    overall_feedback = 'Revise the result'
                 WHERE review_id = 'review:evidence';
                 UPDATE task_review_criteria SET outcome = 'fail'
                 WHERE review_id = 'review:evidence';
                 UPDATE tasks SET stage_id = 'stage:personal:doing'
                 WHERE task_id = 'task:evidence';",
            )?;
            Ok(())
        })
        .await
        .expect("seed exhausted review round");
    let envelope = store
        .load_work_reconciliation_snapshot(&TaskId::new("task:evidence").expect("task id"))
        .await
        .expect("snapshot read")
        .expect("snapshot");
    assert!(envelope.snapshot.review_requested_changes);
    assert!(envelope.snapshot.review_rounds_exhausted);
}

#[tokio::test]
async fn reconciliation_reads_nonretryable_failure_from_validated_ledger_event() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_evidence_task(&store, false).await;
    store
        .with_connection(|connection| {
            connection.execute_batch(
                r#"UPDATE tasks SET latest_submission_id = NULL, latest_review_id = NULL
                 WHERE task_id = 'task:evidence';
                 UPDATE agent_runs SET status = 'failed', error_code = 'permanent_failure',
                    error_message = 'Permanent failure', ended_at = '2026-03-03T00:00:00.000Z'
                 WHERE run_id = 'run:executor';
                 INSERT INTO work_events (
                    event_id, event_kind, workspace_id, task_id, run_id,
                    actor_id, correlation_id, payload_json
                 ) VALUES (
                    'event:failure', 'run.failed', 'workspace:personal', 'task:evidence',
                    'run:executor', 'actor:system', 'correlation:failure',
                    '{"v":1,"run_kind":"executor","generation":1,"attempt_index":0,"error_code":"permanent_failure","retryable":false}'
                 );"#,
            )?;
            Ok(())
        })
        .await
        .expect("seed nonretryable failure");
    let envelope = store
        .load_work_reconciliation_snapshot(&TaskId::new("task:evidence").expect("task id"))
        .await
        .expect("snapshot read")
        .expect("snapshot");
    let failed = envelope.snapshot.failed_run.expect("failed run facts");
    assert!(!failed.retryable);
    assert!(!failed.retries_exhausted);
    assert_eq!(
        failed.recovery_reason,
        Some(noema_tasks::TaskRecoveryReason::InvariantFault)
    );
}
