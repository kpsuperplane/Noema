use noema_tasks::TaskId;
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::params;

use super::{seed_work_events, task_projections};
use crate::{
    StoreError, WorkContractCursor, WorkContractHistoryQuery, WorkEventBeforeQuery,
    WorkGateHistoryQuery, WorkMessageHistoryQuery, WorkOverviewQuery, WorkPageSize,
    WorkReviewHistoryQuery, WorkRunHistoryQuery, WorkRunItemCursor, WorkRunItemOwnerScope,
    WorkRunItemQuery, WorkSubmissionHistoryQuery, test_support::open_ephemeral_store,
};

fn workspace_id() -> WorkspaceId {
    WorkspaceId::new("workspace:personal").expect("workspace id")
}

fn task_id(value: &str) -> TaskId {
    TaskId::new(value).expect("task id")
}

fn page_size(value: u32) -> WorkPageSize {
    WorkPageSize::new(value).expect("page size")
}

#[tokio::test]
async fn workflow_and_project_overview_preserve_stage_order_and_counts() {
    let store = open_ephemeral_store().await.expect("open store");
    task_projections::seed_task_cards(&store).await;
    store
        .with_connection(|connection| {
            connection.execute(
                "INSERT INTO projects (project_id, workspace_id, name, description)
                 VALUES ('project:overview', 'workspace:personal', 'Overview', '')",
                [],
            )?;
            connection.execute(
                "UPDATE tasks SET project_id = 'project:overview'
                 WHERE task_id IN ('task:a', 'task:b', 'task:waiting')",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("scope overview tasks");

    let workflows = store
        .list_work_workflows(&workspace_id())
        .await
        .expect("workflows");
    assert_eq!(workflows.len(), 1);
    assert_eq!(
        workflows[0]
            .stages
            .iter()
            .map(|stage| stage.ordinal)
            .collect::<Vec<_>>(),
        [10, 20, 30, 40, 50, 60, 70]
    );
    let overview = store
        .work_overview(WorkOverviewQuery {
            workspace_id: workspace_id(),
            project_id: Some(ProjectId::new("project:overview").expect("project id")),
            first: page_size(10),
        })
        .await
        .expect("overview");
    assert_eq!(
        overview
            .active_stage_counts
            .iter()
            .map(|count| (count.stage.ordinal, count.task_count))
            .collect::<Vec<_>>(),
        [(10, 1), (20, 0), (30, 1), (40, 1), (50, 0)]
    );
    assert_eq!(overview.needs_you_count, 1);
    assert_eq!(overview.recent_tasks.edges.len(), 3);
    assert!(overview.recent_tasks.edges.iter().all(|edge| {
        edge.node.project.as_ref().is_some_and(|project| {
            project.project_id.as_str() == "project:overview" && project.name == "Overview"
        })
    }));
}

#[tokio::test]
async fn history_connections_hydrate_evidence_and_reject_cursor_reuse() {
    let store = open_ephemeral_store().await.expect("open store");
    task_projections::seed_evidence_task(&store, true).await;
    store
        .with_connection(|connection| {
            connection.execute_batch(
                "INSERT INTO artifacts (
                    artifact_id, owner_object_type, owner_object_id, title, artifact_kind,
                    storage_kind, current_version_id, created_by_actor_id
                 ) VALUES (
                    'artifact:evidence', 'task', 'task:evidence', 'Evidence', 'document',
                    'local_file', 'artifact_version:evidence', 'actor:system'
                 );
                 INSERT INTO artifact_versions (
                    artifact_version_id, artifact_id, version_index, local_relative_path,
                    created_by_actor_id
                 ) VALUES (
                    'artifact_version:evidence', 'artifact:evidence', 1,
                    'tasks/evidence.md', 'actor:system'
                 );
                 INSERT INTO task_submission_artifacts (
                    submission_id, ordinal, artifact_id, artifact_version_id
                 ) VALUES (
                    'submission:evidence', 1, 'artifact:evidence',
                    'artifact_version:evidence'
                 );
                 INSERT INTO task_gates (
                    gate_id, task_id, task_generation, contract_id, gate_kind, gate_state,
                    prompt_markdown, opened_by_actor_id, originating_run_id
                 ) VALUES (
                    'gate:history', 'task:evidence', 1, 'contract:evidence',
                    'clarification', 'open', 'Clarify', 'actor:system', 'run:executor'
                 );
                 INSERT INTO task_messages (
                    message_id, task_id, task_generation, contract_id, gate_id,
                    message_kind, body_markdown, author_actor_id
                 ) VALUES (
                    'task_message:history', 'task:evidence', 1, 'contract:evidence',
                    'gate:history', 'human_answer', 'Answer', 'actor:human'
                 );
                 UPDATE task_gates SET gate_state = 'resolved',
                    resolved_by_actor_id = 'actor:human',
                    resolution_message_id = 'task_message:history',
                    resolved_at = '2026-03-03T00:00:00.000Z'
                 WHERE gate_id = 'gate:history';",
            )?;
            Ok(())
        })
        .await
        .expect("seed history evidence");
    let id = task_id("task:evidence");
    let contracts = store
        .list_work_task_contracts(WorkContractHistoryQuery {
            task_id: id.clone(),
            first: page_size(20),
            after: None,
        })
        .await
        .expect("contracts");
    assert_eq!(contracts.edges[0].node.criteria.len(), 1);
    let submissions = store
        .list_work_task_submissions(WorkSubmissionHistoryQuery {
            task_id: id.clone(),
            first: page_size(20),
            after: None,
        })
        .await
        .expect("submissions");
    assert_eq!(submissions.edges[0].node.criteria.len(), 1);
    assert_eq!(submissions.edges[0].node.artifacts.len(), 1);
    assert_eq!(
        submissions.edges[0].node.artifacts[0]
            .version
            .artifact_version_id,
        "artifact_version:evidence"
    );
    assert_eq!(
        store
            .list_work_task_reviews(WorkReviewHistoryQuery {
                task_id: id.clone(),
                first: page_size(20),
                after: None,
            })
            .await
            .expect("reviews")
            .edges[0]
            .node
            .criteria
            .len(),
        1
    );
    assert_eq!(
        store
            .list_work_task_gates(WorkGateHistoryQuery {
                task_id: id.clone(),
                first: page_size(20),
                after: None,
            })
            .await
            .expect("gates")
            .edges
            .len(),
        1
    );
    assert_eq!(
        store
            .list_work_task_messages(WorkMessageHistoryQuery {
                task_id: id.clone(),
                first: page_size(20),
                after: None,
            })
            .await
            .expect("messages")
            .edges
            .len(),
        1
    );

    let first_runs = store
        .list_work_task_runs(WorkRunHistoryQuery {
            task_id: id.clone(),
            first: page_size(1),
            after: None,
        })
        .await
        .expect("first run page");
    assert!(first_runs.page_info.has_next_page);
    let cursor = crate::WorkRunCursor::decode(
        first_runs
            .page_info
            .end_cursor
            .as_deref()
            .expect("run cursor"),
    )
    .expect("decode run cursor");
    let second_runs = store
        .list_work_task_runs(WorkRunHistoryQuery {
            task_id: id.clone(),
            first: page_size(1),
            after: Some(cursor),
        })
        .await
        .expect("second run page");
    assert_ne!(
        first_runs.edges[0].node.run_id,
        second_runs.edges[0].node.run_id
    );

    let contract_cursor =
        WorkContractCursor::decode(&contracts.edges[0].cursor).expect("contract cursor");
    let error = store
        .list_work_task_contracts(WorkContractHistoryQuery {
            task_id: task_id("task:other"),
            first: page_size(20),
            after: Some(contract_cursor),
        })
        .await
        .expect_err("cursor must be task-bound");
    assert!(matches!(
        error,
        StoreError::Work(noema_tasks::WorkDomainError::InvalidInput {
            field: "work_history.cursor",
            ..
        })
    ));
}

#[tokio::test]
async fn run_item_pages_walk_backward_but_return_chronological_windows() {
    let store = open_ephemeral_store().await.expect("open store");
    task_projections::seed_evidence_task(&store, true).await;
    store
        .with_connection(|connection| {
            for sequence in 1..=5 {
                connection.execute(
                    "INSERT INTO agent_run_items (
                        item_id, run_id, sequence_index, round_index, kind, status, payload_json
                     ) VALUES (?1, 'run:reviewer', ?2, 0, 'progress_notice', 'completed', '{}')",
                    params![format!("run_item:{sequence}"), sequence],
                )?;
            }
            Ok(())
        })
        .await
        .expect("seed run items");
    let owner = WorkRunItemOwnerScope {
        workspace_id: workspace_id(),
        task_id: Some(task_id("task:evidence")),
    };
    let first = store
        .list_work_run_items(WorkRunItemQuery {
            owner: owner.clone(),
            run_id: "run:reviewer".to_string(),
            first: page_size(2),
            before: None,
        })
        .await
        .expect("first item page");
    assert_eq!(
        first
            .edges
            .iter()
            .map(|edge| edge.node.sequence_index)
            .collect::<Vec<_>>(),
        [4, 5]
    );
    let before =
        WorkRunItemCursor::decode(first.page_info.end_cursor.as_deref().expect("item cursor"))
            .expect("decode item cursor");
    let second = store
        .list_work_run_items(WorkRunItemQuery {
            owner,
            run_id: "run:reviewer".to_string(),
            first: page_size(2),
            before: Some(before),
        })
        .await
        .expect("second item page");
    assert_eq!(
        second
            .edges
            .iter()
            .map(|edge| edge.node.sequence_index)
            .collect::<Vec<_>>(),
        [2, 3]
    );
}

#[tokio::test]
async fn activity_is_descending_exclusive_and_latest_cursors_are_exact() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_work_events(&store).await;
    let page = store
        .list_work_events_before(WorkEventBeforeQuery {
            workspace_id: workspace_id(),
            project_id: None,
            task_id: None,
            run_id: None,
            before: None,
            first: page_size(2),
        })
        .await
        .expect("descending activity");
    assert_eq!(
        page.edges
            .iter()
            .map(|edge| edge.node.event_sequence)
            .collect::<Vec<_>>(),
        [3, 2]
    );
    let before = crate::WorkEventCursor::decode(
        page.page_info
            .end_cursor
            .as_deref()
            .expect("activity cursor"),
    )
    .expect("decode event cursor");
    let older = store
        .list_work_events_before(WorkEventBeforeQuery {
            workspace_id: workspace_id(),
            project_id: None,
            task_id: None,
            run_id: None,
            before: Some(before),
            first: page_size(2),
        })
        .await
        .expect("older activity");
    assert_eq!(older.edges[0].node.event_sequence, 1);
    assert_eq!(
        store
            .latest_work_event_cursor(&workspace_id())
            .await
            .expect("workspace high water")
            .expect("workspace cursor")
            .sequence(),
        3
    );
    assert_eq!(
        store
            .latest_task_work_event_cursor(&workspace_id(), &task_id("task:b"))
            .await
            .expect("task high water")
            .expect("task cursor")
            .sequence(),
        3
    );
}
