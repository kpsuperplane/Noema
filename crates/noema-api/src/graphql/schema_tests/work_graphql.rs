use crate::graphql::RequestPrincipal;

fn assert_error_code(response: &async_graphql::Response, message: &str, code: &str) {
    assert_single_graphql_error(response, message);
    assert_eq!(
        response.errors[0]
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code")),
        Some(&async_graphql::Value::from(code))
    );
}

fn response_json(response: async_graphql::Response, context: &str) -> serde_json::Value {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().expect(context)
}

fn assert_json_values(data: &serde_json::Value, expected: &[(&str, serde_json::Value)]) {
    for (pointer, expected) in expected {
        assert_eq!(data.pointer(pointer), Some(expected), "{pointer}");
    }
}

#[test]
fn work_schema_exposes_semantic_operations_without_task_status_aliases() {
    let sdl = build_schema(GraphqlState::for_schema_definition()).sdl();
    for operation in [
        "workOverview",
        "workTasks",
        "projects",
        "needsYou",
        "workActivity",
        "taskHistory",
        "taskRunItems",
        "captureTask",
        "queueTask",
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
        "completedResult: TaskSubmission",
        "messages: [TaskMessage!]!",
        "runs: [TaskRun!]!",
        "submissions: [TaskSubmission!]!",
        "reviews: [TaskReview!]!",
        "artifacts: [Artifact!]!",
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
    assert!(!sdl.contains("REVIEW_READY"));
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
    assert_error_code(&response, "invalid work cursor", "invalid_cursor");
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
        assert_error_code(&response, "invalid work cursor", "invalid_cursor");
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
        assert_error_code(&response, "work is unavailable", "work_unavailable");
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
    assert_error_code(
        &response,
        "workflow filter is inconsistent",
        "workflow_mismatch",
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
                  title
                  description
                  stage { key behavior }
                  revision
                  generation
                  completedResult { submissionId }
                  artifacts { artifactId }
                }
                eventCursor
                clientMutationId
              }
            }"#,
        )
        .await;
    let data = response_json(response, "capture response JSON");
    assert_json_values(
        &data,
        &[
            ("/captureTask/clientMutationId", json!("capture-work-graphql-test")),
            ("/captureTask/task/stage/key", json!("inbox")),
            ("/captureTask/task/description", json!("A durable capture")),
            ("/captureTask/task/revision", json!(1)),
            ("/captureTask/task/generation", json!(1)),
            ("/captureTask/task/completedResult", serde_json::Value::Null),
            ("/captureTask/task/artifacts", json!([])),
        ],
    );
    assert!(
        data["captureTask"]["eventCursor"]
            .as_str()
            .is_some_and(|cursor| !cursor.is_empty())
    );
}

#[tokio::test]
async fn task_gate_uses_the_unified_human_intervention_projection_until_resolved() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
    let capture = schema
        .execute(
            r#"mutation {
              captureTask(input: {
                workspaceId: "workspace:personal"
                title: "Intervention projection"
                clientMutationId: "capture-intervention-projection"
              }) { task { taskId } }
            }"#,
        )
        .await;
    let capture = response_json(capture, "intervention capture JSON");
    let task_id = capture["captureTask"]["task"]["taskId"]
        .as_str()
        .expect("captured task id")
        .to_string();
    let gate_task_id = task_id.clone();
    store
        .with_connection(move |connection| {
            connection.execute(
                "INSERT INTO task_gates (gate_id, task_id, task_generation, gate_kind, gate_state, prompt_markdown, context_markdown, opened_by_actor_id) VALUES ('gate:intervention-projection', ?1, 1, 'clarification', 'open', 'Which direction should the task take?', 'Choose the safest supported direction.', 'agent:test')",
                [&gate_task_id],
            )?;
            connection.execute(
                "UPDATE tasks SET stage_id = 'stage:personal:waiting', active_gate_id = 'gate:intervention-projection', revision = 2 WHERE task_id = ?1",
                [&gate_task_id],
            )?;
            Ok(())
        })
        .await
        .expect("open test gate");

    let pending = schema
        .execute(format!(
            r#"query {{
              pendingHumanInterventions(taskId: "{task_id}") {{
                __typename
                ... on TaskAttention {{
                  kind title summary validActions
                  gate {{ gateId kind prompt contextMarkdown }}
                  task {{ taskId title revision generation validActions activeGate {{ gateId kind }} }}
                }}
              }}
            }}"#
        ))
        .await;
    let pending = response_json(pending, "pending task intervention JSON");
    let intervention = &pending["pendingHumanInterventions"][0];
    assert_json_values(
        intervention,
        &[
            ("/__typename", json!("TaskAttention")),
            ("/kind", json!("CLARIFICATION_REQUIRED")),
            ("/gate/gateId", json!("gate:intervention-projection")),
            ("/gate/prompt", json!("Which direction should the task take?")),
            ("/task/taskId", json!(task_id)),
            ("/task/revision", json!(2)),
            ("/task/generation", json!(1)),
        ],
    );
    assert_eq!(intervention["validActions"], json!(["ANSWER", "CANCEL"]));

    let answer = schema
        .execute(format!(
            r#"mutation {{
              answerTask(input: {{
                taskId: "{task_id}"
                expectedRevision: 2
                expectedGeneration: 1
                clientMutationId: "answer-intervention-projection"
                gateId: "gate:intervention-projection"
                answerMarkdown: "Take the supported route."
              }}) {{ task {{ taskId revision activeGate {{ gateId }} }} }}
            }}"#
        ))
        .await;
    response_json(answer, "resolved task intervention JSON");

    let resolved = schema
        .execute(format!(
            r#"query {{ pendingHumanInterventions(taskId: "{task_id}") {{ __typename }} }}"#
        ))
        .await;
    let resolved = response_json(resolved, "resolved intervention query JSON");
    assert_eq!(resolved["pendingHumanInterventions"], json!([]));
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
    let original = response_json(original, "original capture JSON");
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
    let update = response_json(update, "update JSON");
    assert_eq!(update["updateInboxTask"]["task"]["title"], "Later title");
    assert_eq!(update["updateInboxTask"]["task"]["revision"], 2);

    let replay = schema.execute(capture).await;
    let replay = response_json(replay, "capture replay JSON");
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
    assert_error_code(
        &divergent,
        "idempotency key conflicts",
        "idempotency_conflict",
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
        let data = response_json(response, "capture JSON");
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
        response_json(response, "update JSON");
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
    let data = response_json(response, "task event JSON");
    assert_eq!(data["taskEvents"]["taskId"], target_task_id);
    assert_ne!(data["taskEvents"]["cursor"], target_capture_cursor);
}
