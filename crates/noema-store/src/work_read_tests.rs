use noema_tasks::{TaskId, WorkDomainError};
use noema_workspaces::{ProjectId, WorkspaceId};
use rusqlite::params;

use crate::{
    NoemaStore, ProjectCursor, ProjectQuery, StoreError, WorkEventCursor, WorkEventQuery,
    WorkPageSize, test_support::open_ephemeral_store,
};

async fn seed_projects(store: &NoemaStore) {
    store
        .with_connection(|conn| {
            for (project_id, name, archived_at, updated_at) in [
                (
                    "project:archived",
                    "Archived",
                    Some("2026-01-04T00:00:00.000Z"),
                    "2026-01-04T00:00:00.000Z",
                ),
                ("project:a", "Alpha", None, "2026-01-03T00:00:00.000Z"),
                ("project:b", "Beta", None, "2026-01-03T00:00:00.000Z"),
                ("project:c", "Gamma", None, "2026-01-02T00:00:00.000Z"),
            ] {
                conn.execute(
                    "INSERT INTO projects (
                        project_id, workspace_id, name, description, revision,
                        archived_at, created_at, updated_at
                     ) VALUES (?1, 'workspace:personal', ?2, '', 1, ?3, ?4, ?4)",
                    params![project_id, name, archived_at, updated_at],
                )?;
            }
            Ok(())
        })
        .await
        .expect("seed projects");
}

fn project_query(include_archived: bool, first: u32, after: Option<ProjectCursor>) -> ProjectQuery {
    ProjectQuery {
        workspace_id: WorkspaceId::new("workspace:personal").expect("valid workspace id"),
        include_archived,
        first: WorkPageSize::new(first).expect("valid page size"),
        after,
    }
}

#[tokio::test]
async fn project_connection_filters_archives_and_uses_exclusive_keysets() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_projects(&store).await;

    let first_page = store
        .list_work_projects(project_query(false, 2, None))
        .await
        .expect("read first page");
    assert_eq!(
        first_page
            .edges
            .iter()
            .map(|edge| edge.node.project_id.as_str())
            .collect::<Vec<_>>(),
        ["project:b", "project:a"]
    );
    assert!(first_page.page_info.has_next_page);
    let cursor = ProjectCursor::decode(
        first_page
            .page_info
            .end_cursor
            .as_deref()
            .expect("end cursor"),
    )
    .expect("decode cursor");

    let second_page = store
        .list_work_projects(project_query(false, 2, Some(cursor)))
        .await
        .expect("read second page");
    assert_eq!(
        second_page
            .edges
            .iter()
            .map(|edge| edge.node.project_id.as_str())
            .collect::<Vec<_>>(),
        ["project:c"]
    );
    assert!(!second_page.page_info.has_next_page);
    assert_eq!(
        second_page.page_info.end_cursor,
        second_page.edges.last().map(|edge| edge.cursor.clone())
    );

    let including_archived = store
        .list_work_projects(project_query(true, 100, None))
        .await
        .expect("read archived projects");
    assert_eq!(
        including_archived
            .edges
            .iter()
            .map(|edge| edge.node.project_id.as_str())
            .collect::<Vec<_>>(),
        ["project:archived", "project:b", "project:a", "project:c"]
    );
}

#[tokio::test]
async fn exact_project_lookup_is_workspace_scoped_and_includes_archived_rows() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_projects(&store).await;
    store
        .with_connection(|conn| {
            conn.execute(
                "INSERT INTO workspaces (workspace_id, name, description, is_personal)
                 VALUES ('workspace:other', 'Other', '', 0)",
                [],
            )?;
            conn.execute(
                "INSERT INTO projects (
                    project_id, workspace_id, name, description, revision,
                    created_at, updated_at
                 ) VALUES ('project:other', 'workspace:other', 'Other project', '', 1,
                    '2026-01-05T00:00:00.000Z', '2026-01-05T00:00:00.000Z')",
                [],
            )?;
            for index in 0..128 {
                let project_id = format!("project:bulk-{index:03}");
                conn.execute(
                    "INSERT INTO projects (
                        project_id, workspace_id, name, description, revision,
                        created_at, updated_at
                     ) VALUES (?1, 'workspace:personal', ?2, '', 1,
                        '2026-01-06T00:00:00.000Z', '2026-01-06T00:00:00.000Z')",
                    params![project_id, format!("Bulk project {index}")],
                )?;
            }
            Ok(())
        })
        .await
        .expect("seed exact lookup rows");

    let workspace = WorkspaceId::new("workspace:personal").expect("valid workspace id");
    let archived = ProjectId::new("project:archived").expect("valid project id");
    assert!(
        store
            .get_work_project(&workspace, &archived)
            .await
            .expect("read archived project")
            .is_some()
    );

    let beyond_first_page = ProjectId::new("project:bulk-127").expect("valid project id");
    assert_eq!(
        store
            .get_work_project(&workspace, &beyond_first_page)
            .await
            .expect("read project beyond first page")
            .expect("project beyond first page")
            .project_id,
        beyond_first_page
    );

    let missing = ProjectId::new("project:missing").expect("valid project id");
    assert!(
        store
            .get_work_project(&workspace, &missing)
            .await
            .expect("read missing project")
            .is_none()
    );

    let foreign = ProjectId::new("project:other").expect("valid project id");
    assert!(
        store
            .get_work_project(&workspace, &foreign)
            .await
            .expect("read foreign project")
            .is_none()
    );
    let other_workspace = WorkspaceId::new("workspace:other").expect("valid workspace id");
    assert!(
        store
            .get_work_project(&other_workspace, &foreign)
            .await
            .expect("read project in owning workspace")
            .is_some()
    );
}

#[tokio::test]
async fn project_cursor_cannot_be_reused_across_filters_or_workspaces() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_projects(&store).await;
    store
        .with_connection(|conn| {
            conn.execute(
                "INSERT INTO workspaces (workspace_id, name, description, is_personal)
                 VALUES ('workspace:other', 'Other', '', 0)",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("seed second workspace");

    let page = store
        .list_work_projects(project_query(false, 1, None))
        .await
        .expect("read cursor source page");
    let cursor = ProjectCursor::decode(page.page_info.end_cursor.as_deref().expect("end cursor"))
        .expect("decode cursor");

    let filter_error = store
        .list_work_projects(project_query(true, 1, Some(cursor.clone())))
        .await
        .expect_err("filter reuse must fail");
    assert_invalid_cursor(filter_error);

    let workspace_error = store
        .list_work_projects(ProjectQuery {
            workspace_id: WorkspaceId::new("workspace:other").expect("valid workspace id"),
            include_archived: false,
            first: WorkPageSize::new(1).expect("valid page size"),
            after: Some(cursor),
        })
        .await
        .expect_err("workspace reuse must fail");
    assert_invalid_cursor(workspace_error);
}

#[tokio::test]
async fn project_connection_rejects_malformed_persisted_records() {
    let store = open_ephemeral_store().await.expect("open store");
    store
        .with_connection(|conn| {
            conn.execute_batch("PRAGMA ignore_check_constraints = ON")?;
            conn.execute(
                "INSERT INTO projects (
                    project_id, workspace_id, name, description, revision,
                    created_at, updated_at
                 ) VALUES (
                    'malformed', 'workspace:personal', 'Malformed', '', 1,
                    '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                 )",
                [],
            )?;
            conn.execute_batch("PRAGMA ignore_check_constraints = OFF")?;
            Ok(())
        })
        .await
        .expect("seed malformed project");

    let error = store
        .list_work_projects(project_query(false, 1, None))
        .await
        .expect_err("malformed record must fail closed");
    assert!(matches!(
        error,
        StoreError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            _
        ))
    ));
}

fn assert_invalid_cursor(error: StoreError) {
    assert!(matches!(
        error,
        StoreError::Work(WorkDomainError::InvalidInput {
            field: "work_project.cursor",
            ref message,
        }) if message == "invalid_cursor"
    ));
}

async fn seed_work_events(store: &NoemaStore) {
    store
        .with_connection(|conn| {
            conn.execute_batch("PRAGMA foreign_keys = OFF")?;
            for (event_id, workspace_id, project_id, task_id, run_id, revision) in [
                (
                    "event:1",
                    "workspace:personal",
                    "project:a",
                    "task:a",
                    "run:alpha",
                    1,
                ),
                (
                    "event:2",
                    "workspace:personal",
                    "project:b",
                    "task:b",
                    "run:beta",
                    2,
                ),
                (
                    "event:3",
                    "workspace:personal",
                    "project:a",
                    "task:b",
                    "run:alpha",
                    3,
                ),
                (
                    "event:4",
                    "workspace:other",
                    "project:a",
                    "task:a",
                    "run:alpha",
                    4,
                ),
            ] {
                conn.execute(
                    "INSERT INTO work_events (
                        event_id, event_kind, workspace_id, project_id, task_id,
                        run_id, actor_id, causation_id, correlation_id, payload_json
                     ) VALUES (
                        ?1, 'task.cancelled', ?2, ?3, ?4, ?5,
                        'actor:test', 'command:test', 'correlation:test', ?6
                     )",
                    params![
                        event_id,
                        workspace_id,
                        project_id,
                        task_id,
                        run_id,
                        format!(
                            "{{\"v\":1,\"revision\":{revision},\"generation\":1,\"reason_present\":false}}"
                        ),
                    ],
                )?;
            }
            conn.execute_batch("PRAGMA foreign_keys = ON")?;
            Ok(())
        })
        .await
        .expect("seed work events");
}

fn event_query(
    first: u32,
    after: Option<WorkEventCursor>,
    project_id: Option<&str>,
    task_id: Option<&str>,
    run_id: Option<&str>,
) -> WorkEventQuery {
    WorkEventQuery {
        workspace_id: WorkspaceId::new("workspace:personal").expect("valid workspace id"),
        project_id: project_id.map(|value| ProjectId::new(value).expect("valid project id")),
        task_id: task_id.map(|value| TaskId::new(value).expect("valid task id")),
        run_id: run_id.map(str::to_string),
        after,
        first: WorkPageSize::new(first).expect("valid page size"),
    }
}

#[tokio::test]
async fn work_event_connection_replays_global_order_and_resumes_exclusively() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_work_events(&store).await;

    let first_page = store
        .list_work_events_after(event_query(2, None, None, None, None))
        .await
        .expect("read first event page");
    assert_eq!(
        first_page
            .edges
            .iter()
            .map(|edge| edge.node.event_sequence)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(
        first_page
            .edges
            .iter()
            .map(|edge| edge.node.task_id.as_ref().expect("task id").as_str())
            .collect::<Vec<_>>(),
        ["task:a", "task:b"]
    );
    assert!(first_page.page_info.has_next_page);
    let after = WorkEventCursor::decode(
        first_page
            .page_info
            .end_cursor
            .as_deref()
            .expect("end cursor"),
    )
    .expect("decode event cursor");
    assert_eq!(after.sequence(), 2);

    let second_page = store
        .list_work_events_after(event_query(2, Some(after), None, None, None))
        .await
        .expect("resume event replay");
    assert_eq!(
        second_page
            .edges
            .iter()
            .map(|edge| edge.node.event_sequence)
            .collect::<Vec<_>>(),
        [3]
    );
    assert!(!second_page.page_info.has_next_page);
}

#[tokio::test]
async fn work_event_connection_applies_workspace_and_optional_scope_filters() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_work_events(&store).await;

    for (project_id, task_id, run_id, expected) in [
        (Some("project:a"), None, None, vec![1, 3]),
        (None, Some("task:b"), None, vec![2, 3]),
        (None, None, Some("run:alpha"), vec![1, 3]),
        (Some("project:b"), Some("task:b"), Some("run:beta"), vec![2]),
        (Some("project:b"), None, Some("run:alpha"), Vec::new()),
    ] {
        let page = store
            .list_work_events_after(event_query(100, None, project_id, task_id, run_id))
            .await
            .expect("read filtered events");
        assert_eq!(
            page.edges
                .iter()
                .map(|edge| edge.node.event_sequence)
                .collect::<Vec<_>>(),
            expected
        );
    }

    let other_workspace = store
        .list_work_events_after(WorkEventQuery {
            workspace_id: WorkspaceId::new("workspace:other").expect("valid workspace id"),
            project_id: None,
            task_id: None,
            run_id: None,
            after: None,
            first: WorkPageSize::new(100).expect("valid page size"),
        })
        .await
        .expect("read other workspace events");
    assert_eq!(
        other_workspace
            .edges
            .iter()
            .map(|edge| edge.node.event_sequence)
            .collect::<Vec<_>>(),
        [4]
    );
}

#[tokio::test]
async fn work_event_connection_rejects_unknown_kinds_and_invalid_payloads() {
    let store = open_ephemeral_store().await.expect("open store");
    seed_work_events(&store).await;
    store
        .with_connection(|conn| {
            conn.execute(
                "UPDATE work_events SET event_kind = 'future.kind' WHERE event_sequence = 1",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("corrupt event kind");
    let kind_error = store
        .list_work_events_after(event_query(1, None, None, None, None))
        .await
        .expect_err("unknown event kind must fail closed");
    assert!(matches!(
        kind_error,
        StoreError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            _
        ))
    ));

    store
        .with_connection(|conn| {
            conn.execute(
                "UPDATE work_events
                 SET event_kind = 'task.cancelled', payload_json = '{\"v\":1}'
                 WHERE event_sequence = 1",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("corrupt event payload");
    let payload_error = store
        .list_work_events_after(event_query(1, None, None, None, None))
        .await
        .expect_err("invalid event payload must fail closed");
    assert!(matches!(
        payload_error,
        StoreError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
            10,
            rusqlite::types::Type::Text,
            _
        ))
    ));
}

#[path = "work_read_surface_tests.rs"]
mod surface;
#[path = "work_read_task_projection_tests.rs"]
mod task_projections;
