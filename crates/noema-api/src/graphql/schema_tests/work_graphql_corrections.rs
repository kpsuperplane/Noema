use crate::graphql::RequestPrincipal;
use noema_store::WorkCommandService;
use noema_tasks::{CaptureTask, CommandMeta, TaskProvenance, TaskSourceKind, WorkCommand};
use noema_workspaces::WorkspaceId;
use std::sync::Arc;
use tokio::sync::Notify;

#[tokio::test]
async fn project_mutation_replay_returns_the_original_committed_snapshot() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let create = r#"mutation {
      createProject(input: {
        workspaceId: "workspace:personal"
        name: "Receipt-stable project"
        description: "Original description"
        clientMutationId: "project-receipt-stability"
      }) {
        project { projectId name description revision updatedAt }
        eventCursor
        clientMutationId
      }
    }"#;
    let original = schema.execute(create).await;
    assert!(original.errors.is_empty(), "{:?}", original.errors);
    let original = original.data.into_json().expect("original project JSON");
    let project_id = original["createProject"]["project"]["projectId"]
        .as_str()
        .expect("created project id");

    let update = schema
        .execute(format!(
            r#"mutation {{
              updateProject(input: {{
                projectId: "{project_id}"
                expectedRevision: 1
                name: "Later project name"
                clientMutationId: "project-update-after-create"
              }}) {{ project {{ name revision }} eventCursor }}
            }}"#
        ))
        .await;
    assert!(update.errors.is_empty(), "{:?}", update.errors);
    let update = update.data.into_json().expect("project update JSON");
    assert_eq!(
        update["updateProject"]["project"]["name"],
        "Later project name"
    );
    assert_eq!(update["updateProject"]["project"]["revision"], 2);

    let replay = schema.execute(create).await;
    assert!(replay.errors.is_empty(), "{:?}", replay.errors);
    assert_eq!(
        replay.data.into_json().expect("project replay JSON")["createProject"],
        original["createProject"]
    );

    let divergent = schema
        .execute(
            r#"mutation {
              createProject(input: {
                workspaceId: "workspace:personal"
                name: "Divergent project"
                description: "Original description"
                clientMutationId: "project-receipt-stability"
              }) { eventCursor }
            }"#,
        )
        .await;
    assert_single_graphql_error(&divergent, "idempotency key conflicts");
    assert_eq!(
        divergent.errors[0]
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code")),
        Some(&async_graphql::Value::from("idempotency_conflict"))
    );
}

#[tokio::test]
async fn project_authorization_finds_rows_beyond_the_first_page_and_archived_rows() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let mut target_project_id = None;

    for index in 0..101 {
        let response = schema
            .execute(format!(
                r#"mutation {{
                  createProject(input: {{
                    workspaceId: "workspace:personal"
                    name: "Authorization project {index}"
                    clientMutationId: "authorization-project-{index}"
                  }}) {{ project {{ projectId }} }}
                }}"#
            ))
            .await;
        assert!(response.errors.is_empty(), "{:#?}", response.errors);
        if index == 100 {
            target_project_id =
                response.data.into_json().expect("target project JSON")["createProject"]["project"]
                    ["projectId"]
                    .as_str()
                    .map(str::to_owned);
        }
    }
    let target_project_id = target_project_id.expect("project beyond first page");

    let archive = schema
        .execute(format!(
            r#"mutation {{
              archiveProject(input: {{
                projectId: "{target_project_id}"
                expectedRevision: 1
                clientMutationId: "archive-project-beyond-page"
              }}) {{ project {{ projectId revision archivedAt }} }}
            }}"#
        ))
        .await;
    assert!(archive.errors.is_empty(), "{:#?}", archive.errors);
    let archived = archive.data.into_json().expect("archived project JSON");
    assert_eq!(
        archived["archiveProject"]["project"]["projectId"],
        target_project_id
    );
    assert_eq!(archived["archiveProject"]["project"]["revision"], 2);
    assert!(archived["archiveProject"]["project"]["archivedAt"].is_string());

    let reopen = schema
        .execute(format!(
            r#"mutation {{
              reopenProject(input: {{
                projectId: "{target_project_id}"
                expectedRevision: 2
                clientMutationId: "reopen-project-beyond-page"
              }}) {{ project {{ projectId revision archivedAt }} }}
            }}"#
        ))
        .await;
    assert!(reopen.errors.is_empty(), "{:#?}", reopen.errors);
    let reopened = reopen.data.into_json().expect("reopened project JSON");
    assert_eq!(
        reopened["reopenProject"]["project"]["projectId"],
        target_project_id
    );
    assert_eq!(reopened["reopenProject"]["project"]["revision"], 3);
    assert!(reopened["reopenProject"]["project"]["archivedAt"].is_null());
}

async fn capture_for_subscription(schema: &GraphqlSchema, key: &str) -> (String, String) {
    let response = schema
        .execute(format!(
            r#"mutation {{
              captureTask(input: {{
                workspaceId: "workspace:personal"
                title: "{key}"
                clientMutationId: "{key}"
              }}) {{ task {{ taskId }} eventCursor }}
            }}"#
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("capture JSON");
    (
        data["captureTask"]["task"]["taskId"]
            .as_str()
            .expect("task id")
            .to_string(),
        data["captureTask"]["eventCursor"]
            .as_str()
            .expect("event cursor")
            .to_string(),
    )
}

#[tokio::test]
async fn work_subscriptions_replay_history_and_absent_after_tails_the_head() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let (_, first_cursor) = capture_for_subscription(&schema, "work-history-first").await;
    let (historical_task_id, historical_cursor) =
        capture_for_subscription(&schema, "work-history-second").await;

    let mut replay = schema.execute_stream(async_graphql::Request::new(format!(
        r#"subscription {{
          workEvents(workspaceId: "workspace:personal", after: "{first_cursor}") {{
            cursor taskId kind
          }}
        }}"#
    )));
    let replayed = replay.next().await.expect("historical replay response");
    assert!(replayed.errors.is_empty(), "{:?}", replayed.errors);
    let replayed = replayed.data.into_json().expect("historical replay JSON");
    assert_eq!(replayed["workEvents"]["taskId"], historical_task_id);
    assert_eq!(replayed["workEvents"]["kind"], "task.captured");
    let replayed_marker = replay.next().await.expect("historical marker response");
    assert!(
        replayed_marker.errors.is_empty(),
        "{:?}",
        replayed_marker.errors
    );
    let replayed_marker = replayed_marker
        .data
        .into_json()
        .expect("historical marker JSON");
    assert_eq!(replayed_marker["workEvents"]["taskId"], historical_task_id);
    assert_eq!(replayed_marker["workEvents"]["cursor"], historical_cursor);

    let mut tail = schema.execute_stream(async_graphql::Request::new(
        r#"subscription {
          workEvents(workspaceId: "workspace:personal") { cursor taskId kind }
        }"#,
    ));
    let pending = tokio::time::timeout(std::time::Duration::from_millis(20), tail.next()).await;
    assert!(
        pending.is_err(),
        "absent after starts beyond the durable head"
    );

    let (live_task_id, live_cursor) = capture_for_subscription(&schema, "work-tail-live").await;
    let live = tokio::time::timeout(std::time::Duration::from_secs(1), tail.next())
        .await
        .expect("live event after tail subscription")
        .expect("live Work response");
    assert!(live.errors.is_empty(), "{:?}", live.errors);
    let live = live.data.into_json().expect("live Work JSON");
    assert_eq!(live["workEvents"]["taskId"], live_task_id);
    assert_eq!(live["workEvents"]["kind"], "task.captured");
    let live_marker = tokio::time::timeout(std::time::Duration::from_secs(1), tail.next())
        .await
        .expect("live notification marker")
        .expect("live marker response");
    assert!(live_marker.errors.is_empty(), "{:?}", live_marker.errors);
    assert_eq!(
        live_marker.data.into_json().expect("live marker JSON")["workEvents"]["cursor"],
        live_cursor
    );
}

#[tokio::test]
async fn work_subscription_handoff_replays_a_commit_exactly_once() {
    let store = crate::test_support::test_store().await;
    let handoff_started = Arc::new(Notify::new());
    let handoff_release = Arc::new(Notify::new());
    let state = GraphqlState::for_tests_with_store(store.clone()).with_work_subscription_handoff({
        let handoff_started = handoff_started.clone();
        let handoff_release = handoff_release.clone();
        move || {
            let handoff_started = handoff_started.clone();
            let handoff_release = handoff_release.clone();
            async move {
                handoff_started.notify_one();
                handoff_release.notified().await;
            }
        }
    });
    let schema = build_schema(state);
    let mut stream = schema.execute_stream(async_graphql::Request::new(
        r#"subscription {
          workEvents(workspaceId: "workspace:personal") { cursor eventId taskId kind }
        }"#,
    ));
    let mut first = Box::pin(stream.next());
    tokio::select! {
        _ = handoff_started.notified() => {}
        response = first.as_mut() => panic!("subscription yielded before the handoff: {response:?}"),
    }
    let committed = WorkCommandService::new(
        store,
        crate::test_support::ready_test_provider_registry(),
    )
    .execute(WorkCommand::CaptureTask(CaptureTask {
        meta: CommandMeta {
            actor_id: "actor:human:local".to_string(),
            causation_id: None,
            correlation_id: "correlation:subscription:handoff".to_string(),
            idempotency_key: Some("subscription:handoff".to_string()),
        },
        workspace_id: WorkspaceId::new("workspace:personal").expect("workspace id"),
        title: "handoff event".to_string(),
        description_markdown: String::new(),
        project_id: None,
        provenance: TaskProvenance {
            source_kind: TaskSourceKind::ChatCapture,
            created_by_actor_id: "actor:human:local".to_string(),
            ..TaskProvenance::default()
        },
    }))
    .await
    .expect("commit handoff event")
    .task
    .expect("captured task");
    handoff_release.notify_one();

    let first = first.await.expect("first Work event");
    assert!(first.errors.is_empty(), "{:#?}", first.errors);
    let first = first.data.into_json().expect("first Work event JSON");
    assert_eq!(first["workEvents"]["taskId"], committed.task_id.as_str());
    assert_eq!(first["workEvents"]["kind"], "task.captured");
    let capture_cursor = first["workEvents"]["cursor"]
        .as_str()
        .expect("capture cursor");
    let capture_event_id = first["workEvents"]["eventId"]
        .as_str()
        .expect("capture event id");
    let second = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
        .await
        .expect("notification marker response")
        .expect("notification marker event");
    assert!(second.errors.is_empty(), "{:#?}", second.errors);
    let second = second.data.into_json().expect("notification marker JSON");
    assert_eq!(second["workEvents"]["taskId"], committed.task_id.as_str());
    assert_eq!(second["workEvents"]["kind"], "notification.queued");
    assert_ne!(second["workEvents"]["cursor"].as_str(), Some(capture_cursor));
    assert_ne!(second["workEvents"]["eventId"].as_str(), Some(capture_event_id));
    let third = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next()).await;
    assert!(
        third.is_err(),
        "the committed events must be delivered exactly once, got a duplicate response"
    );
}

#[tokio::test]
async fn work_subscription_authorizes_before_decoding_a_cursor() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    for subscription in [
        r#"subscription {
          workEvents(workspaceId: "workspace:personal", after: "malformed") { cursor }
        }"#,
        r#"subscription {
          taskEvents(taskId: "malformed", after: "malformed") { cursor }
        }"#,
    ] {
        let mut stream = schema.execute_stream(async_graphql::Request::new(subscription).data(
            RequestPrincipal {
                subject_id: "human:foreign",
            },
        ));
        let response = stream.next().await.expect("authorization response");
        assert_single_graphql_error(&response, "work is unavailable");
        assert_eq!(
            response.errors[0]
                .extensions
                .as_ref()
                .and_then(|extensions| extensions.get("code")),
            Some(&async_graphql::Value::from("work_unavailable"))
        );
    }
}

mod recovery_projections {
    use super::*;
    include!("work_graphql_corrections_recovery.rs");
}
