use crate::graphql::RequestPrincipal;

#[test]
fn work_schema_exposes_semantic_operations_without_task_status_aliases() {
    let sdl = build_schema(GraphqlState::for_schema_definition()).sdl();
    for operation in [
        "workOverview",
        "workTasks",
        "projects",
        "needsYou",
        "workActivity",
        "completedTasks",
        "taskRunItems",
        "captureTask",
        "queueTask",
        "requestTaskChanges",
        "reopenTask",
        "workEvents",
        "taskEvents",
    ] {
        assert!(
            sdl.contains(operation),
            "missing Work operation {operation}"
        );
    }
    assert!(sdl.contains("stage: WorkflowStage!"));
    assert!(!sdl.contains("executionPhase:"));
    assert!(!sdl.contains("setTaskStage"));
}

#[test]
fn work_schema_exposes_exact_detail_attention_and_closed_vocabularies() {
    let sdl = build_schema(GraphqlState::for_schema_definition()).sdl();
    for enum_name in [
        "TaskAttentionKind",
        "TaskGateKind",
        "TaskGateState",
        "TaskRecoveryReason",
        "TaskMessageKind",
        "TaskRunKind",
        "TaskRunStatus",
        "TaskReviewVerdict",
        "TaskCriterionOutcome",
        "TaskRunItemKind",
        "TaskRunItemStatus",
    ] {
        assert!(
            sdl.contains(&format!("enum {enum_name}")),
            "missing {enum_name}"
        );
    }
    for field in [
        "descriptionPreview: String!",
        "acceptedResult: TaskSubmission",
        "artifacts(first: Int, after: String): TaskArtifactConnection!",
        "task: TaskCard!",
        "gate: TaskGate",
        "review: TaskReview",
    ] {
        assert!(sdl.contains(field), "missing schema field {field}");
    }
    assert!(
        sdl.contains("kind: String!"),
        "WorkEvent.kind stays dotted text"
    );
    assert!(sdl.contains("CLARIFICATION_REQUIRED"));
    assert!(sdl.contains("REVIEW_READY"));
}

#[tokio::test]
async fn work_reads_require_an_authenticated_owner() {
    let schema = build_schema_without_request_principal(GraphqlState::for_tests());
    let response = schema
        .execute(
            r#"query {
              workTasks(input: { workspaceId: "workspace:personal" }) {
                edges { node { taskId } }
              }
            }"#,
        )
        .await;
    assert_single_graphql_error(&response, "request is unauthenticated");
}

#[tokio::test]
async fn work_connections_reject_malformed_cursors_with_stable_code() {
    let schema = build_schema(GraphqlState::for_tests());
    let response = schema
        .execute(
            r#"query {
              projects(workspaceId: "workspace:personal", after: "malformed") {
                edges { node { projectId } }
              }
            }"#,
        )
        .await;
    assert_single_graphql_error(&response, "invalid work cursor");
    assert_eq!(
        response.errors[0]
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code")),
        Some(&async_graphql::Value::from("invalid_cursor"))
    );
}

#[tokio::test]
async fn work_connections_enforce_page_bounds_before_store_reads() {
    let schema = build_schema(GraphqlState::for_tests());
    for first in [0, 101] {
        let response = schema
            .execute(format!(
                r#"query {{
                  projects(workspaceId: "workspace:personal", first: {first}) {{
                    edges {{ node {{ projectId }} }}
                  }}
                }}"#
            ))
            .await;
        assert_eq!(
            response.errors[0]
                .extensions
                .as_ref()
                .and_then(|extensions| extensions.get("code")),
            Some(&async_graphql::Value::from("invalid_cursor"))
        );
    }
}

#[tokio::test]
async fn work_authorization_is_indistinguishable_before_identifier_validation() {
    let schema = build_schema(GraphqlState::for_tests());
    for query in [
        r#"query { task(taskId: "malformed") { taskId } }"#,
        r#"query { task(taskId: "task:missing") { taskId } }"#,
        r#"mutation {
          queueTask(input: {
            taskId: "malformed"
            expectedRevision: 1
            expectedGeneration: 1
            clientMutationId: "foreign-queue"
          }) { eventCursor }
        }"#,
    ] {
        let response = schema
            .execute(async_graphql::Request::new(query).data(RequestPrincipal {
                subject_id: "human:foreign",
            }))
            .await;
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

#[tokio::test]
async fn work_scope_filters_reject_semantic_stage_conflicts() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let response = schema
        .execute(
            r#"query {
              workTasks(input: {
                workspaceId: "workspace:personal"
                scope: ACTIVE
                stageBehaviors: [TERMINAL_SUCCESS]
              }) { edges { node { taskId } } }
            }"#,
        )
        .await;
    assert_single_graphql_error(&response, "workflow filter is inconsistent");
    assert_eq!(
        response.errors[0]
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code")),
        Some(&async_graphql::Value::from("workflow_mismatch"))
    );
}

#[tokio::test]
async fn semantic_work_mutations_require_client_idempotency_keys() {
    let schema = build_schema(GraphqlState::for_tests());
    let response = schema
        .execute(
            r#"mutation {
              captureTask(input: {
                workspaceId: "workspace:personal"
                title: "Capture without idempotency key"
              }) { clientMutationId }
            }"#,
        )
        .await;
    assert!(!response.errors.is_empty());
    assert!(response.errors[0].message.contains("clientMutationId"));
}

#[tokio::test]
async fn semantic_work_mutations_reject_whitespace_idempotency_aliases() {
    let schema = build_schema(GraphqlState::for_tests());
    for client_mutation_id in [" leading", "trailing ", " surrounded "] {
        let response = schema
            .execute(format!(
                r#"mutation {{
                  captureTask(input: {{
                    workspaceId: "workspace:personal"
                    title: "Whitespace key"
                    clientMutationId: "{client_mutation_id}"
                  }}) {{ clientMutationId }}
                }}"#
            ))
            .await;
        assert_single_graphql_error(&response, "invalid clientMutationId");
    }
}

#[tokio::test]
async fn capture_task_returns_authoritative_work_projection() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let response = schema
        .execute(
            r#"mutation {
              captureTask(input: {
                workspaceId: "workspace:personal"
                title: "Capture through Work"
                description: "A durable capture"
                clientMutationId: "capture-work-graphql-test"
              }) {
                task {
                  taskId
                  workspace { workspaceId isPersonal }
                  title
                  description
                  descriptionPreview
                  stage { key behavior }
                  revision
                  generation
                  acceptedResult { submissionId }
                  artifacts { edges { node { artifactId } } pageInfo { hasNextPage } }
                }
                eventCursor
                clientMutationId
              }
            }"#,
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("capture response JSON");
    assert_eq!(
        data["captureTask"]["clientMutationId"],
        "capture-work-graphql-test"
    );
    assert!(
        data["captureTask"]["eventCursor"]
            .as_str()
            .is_some_and(|cursor| !cursor.is_empty())
    );
    assert_eq!(
        data["captureTask"]["task"]["workspace"]["workspaceId"],
        "workspace:personal"
    );
    assert_eq!(data["captureTask"]["task"]["workspace"]["isPersonal"], true);
    assert_eq!(data["captureTask"]["task"]["stage"]["key"], "inbox");
    assert_eq!(
        data["captureTask"]["task"]["descriptionPreview"],
        "A durable capture"
    );
    assert_eq!(data["captureTask"]["task"]["revision"], 1);
    assert_eq!(data["captureTask"]["task"]["generation"], 1);
    assert!(data["captureTask"]["task"]["acceptedResult"].is_null());
    assert_eq!(data["captureTask"]["task"]["artifacts"]["edges"], json!([]));
}

#[tokio::test]
async fn task_mutation_replay_returns_the_original_committed_detail() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let capture = r#"mutation {
      captureTask(input: {
        workspaceId: "workspace:personal"
        title: "Receipt-stable title"
        description: "Receipt-stable description"
        clientMutationId: "capture-receipt-stability"
      }) {
        task { taskId title description revision generation updatedAt }
        eventCursor
        clientMutationId
      }
    }"#;
    let original = schema.execute(capture).await;
    assert!(original.errors.is_empty(), "{:?}", original.errors);
    let original = original.data.into_json().expect("original capture JSON");
    let task_id = original["captureTask"]["task"]["taskId"]
        .as_str()
        .expect("captured task id");

    let update = schema
        .execute(format!(
            r#"mutation {{
              updateInboxTask(input: {{
                taskId: "{task_id}"
                expectedRevision: 1
                expectedGeneration: 1
                title: "Later title"
                clientMutationId: "update-after-capture"
              }}) {{ task {{ title revision }} eventCursor }}
            }}"#
        ))
        .await;
    assert!(update.errors.is_empty(), "{:?}", update.errors);
    let update = update.data.into_json().expect("update JSON");
    assert_eq!(update["updateInboxTask"]["task"]["title"], "Later title");
    assert_eq!(update["updateInboxTask"]["task"]["revision"], 2);

    let replay = schema.execute(capture).await;
    assert!(replay.errors.is_empty(), "{:?}", replay.errors);
    let replay = replay.data.into_json().expect("capture replay JSON");
    assert_eq!(replay["captureTask"], original["captureTask"]);

    let divergent = schema
        .execute(
            r#"mutation {
              captureTask(input: {
                workspaceId: "workspace:personal"
                title: "Divergent title"
                description: "Receipt-stable description"
                clientMutationId: "capture-receipt-stability"
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
async fn task_subscription_recovers_from_wakeup_lag_and_keeps_its_task_filter() {
    let store = crate::test_support::test_store().await;
    let state = GraphqlState::for_tests_with_store(store);
    let subscriptions = state.subscriptions().clone();
    let schema = build_schema(state);

    async fn capture(schema: &GraphqlSchema, key: &str) -> (String, String) {
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

    async fn update(schema: &GraphqlSchema, task_id: &str, key: &str) {
        let response = schema
            .execute(format!(
                r#"mutation {{
                  updateInboxTask(input: {{
                    taskId: "{task_id}"
                    expectedRevision: 1
                    expectedGeneration: 1
                    title: "updated {key}"
                    clientMutationId: "update-{key}"
                  }}) {{ eventCursor }}
                }}"#
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
    }

    let (target_task_id, target_capture_cursor) = capture(&schema, "subscription-target").await;
    let (other_task_id, _) = capture(&schema, "subscription-other").await;
    let mut stream = schema.execute_stream(async_graphql::Request::new(format!(
        r#"subscription {{
          taskEvents(taskId: "{target_task_id}", after: "{target_capture_cursor}") {{
            cursor taskId kind
          }}
        }}"#
    )));

    let pending = tokio::time::timeout(std::time::Duration::from_millis(20), stream.next()).await;
    assert!(
        pending.is_err(),
        "the filtered subscription starts at its high-water mark"
    );

    for _ in 0..300 {
        subscriptions.publish_work(noema_runtime::WorkRuntimeEvent::Committed {
            workspace_id: "workspace:personal".to_string(),
            task_id: Some(other_task_id.clone()),
        });
    }
    update(&schema, &other_task_id, "subscription-other").await;
    update(&schema, &target_task_id, "subscription-target").await;

    let response = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
        .await
        .expect("durable replay after wakeup lag")
        .expect("task event response");
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("task event JSON");
    assert_eq!(data["taskEvents"]["taskId"], target_task_id);
    assert_ne!(data["taskEvents"]["cursor"], target_capture_cursor);
}
