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
fn tasks_schema_exposes_semantic_operations_without_task_status_aliases() {
    let sdl = build_schema(GraphqlState::for_schema_definition()).sdl();
    for operation in [
        "tasksOverview",
        "tasks",
        "projects",
        "needsYou",
        "tasksActivity",
        "taskHistory",
        "taskRunItems",
        "taskWorkspaceFile",
        "captureTask",
        "queueTask",
        "scheduleTask",
        "taskRecurrence",
        "taskRecurrences",
        "taskSchedulePreview",
        "reopenTask",
        "tasksEvents",
        "taskEvents",
    ] {
        assert!(
            sdl.contains(operation),
            "missing Tasks operation {operation}"
        );
    }
    assert!(sdl.contains("stage: WorkflowStage!"));
    assert!(!sdl.contains("executionPhase:"));
    assert!(!sdl.contains("setTaskStage"));
}

#[tokio::test]
async fn recurrence_list_does_not_depend_on_an_active_task_instance() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let captured = response_json(
        schema
            .execute(
                r#"mutation {
                  captureTask(input: {
                    workspaceId: "workspace:personal"
                    title: "Show positive news"
                    schedule: {
                      scheduledFor: "2030-01-01T08:00:00Z"
                      timeZone: "UTC"
                      recurrence: { startsAt: "2030-01-01T08:00:00Z", cronExpression: "0 8 * * *" }
                    }
                    clientMutationId: "recurrence-list-capture"
                  }) {
                    task { taskId revision generation }
                  }
                }"#,
            )
            .await,
        "recurring capture",
    );
    let task = &captured["captureTask"]["task"];
    let cancelled = schema
        .execute(format!(
            r#"mutation {{
              cancelTask(input: {{
                taskId: "{}"
                expectedRevision: {}
                expectedGeneration: {}
                clientMutationId: "recurrence-list-cancel"
              }}) {{ task {{ taskId }} }}
            }}"#,
            task["taskId"].as_str().expect("task id"),
            task["revision"].as_i64().expect("task revision"),
            task["generation"].as_i64().expect("task generation"),
        ))
        .await;
    assert!(cancelled.errors.is_empty(), "{:?}", cancelled.errors);

    let listed = response_json(
        schema
            .execute(
                r#"query {
                  tasks(input: { workspaceId: "workspace:personal", scope: ACTIVE }) {
                    edges { node { taskId } }
                  }
                  taskRecurrences(workspaceId: "workspace:personal") {
                    recurrenceId title lifecycle nextRunAt
                  }
                }"#,
            )
            .await,
        "recurrence list",
    );
    assert_eq!(listed["tasks"]["edges"], json!([]));
    assert_eq!(listed["taskRecurrences"].as_array().map(Vec::len), Some(1));
    assert_json_values(
        &listed,
        &[
            ("/taskRecurrences/0/title", json!("Show positive news")),
            ("/taskRecurrences/0/lifecycle", json!("ACTIVE")),
            (
                "/taskRecurrences/0/nextRunAt",
                json!("2030-01-02T08:00:00Z"),
            ),
        ],
    );
}

#[test]
fn tasks_schema_exposes_exact_detail_attention_and_closed_vocabularies() {
    let sdl = build_schema(GraphqlState::for_schema_definition()).sdl();
    for enum_name in [
        "TaskAttentionKind",
        "TaskGateKind",
        "TaskGateState",
        "TaskRecoveryReason",
        "TaskRunKind",
        "TaskRunStatus",
        "TaskRunItemKind",
        "TaskRunItemStatus",
    ] {
        assert!(
            sdl.contains(&format!("enum {enum_name}")),
            "missing {enum_name}"
        );
    }
    for field in [
        "taskDocumentPreview: String!",
        "taskDocument: String!",
        "taskDocumentDigest: String!",
        "resultDocument: String",
        "resultMetadata: JSON!",
        "reviewDocument: String",
        "workspaceFiles: [TaskWorkspaceFile!]!",
        "workspaceFilesTruncated: Boolean!",
        "taskWorkspaceFile(taskId: String!, path: String!): TaskWorkspaceFileText!",
        "messages: [TaskMessage!]!",
        "runs: [TaskRun!]!",
        "contributorInstanceNames: [String!]!",
        "artifacts: [Artifact!]!",
        "task: TaskCard!",
        "gate: TaskGate",
    ] {
        assert!(sdl.contains(field), "missing schema field {field}");
    }
    for removed in [
        "TaskExecutionContract",
        "TaskSubmission",
        "TaskReview",
        "TaskValidationCriterionInput",
        "contractId:",
        "triggeringSubmissionId:",
        "triggeringReviewId:",
        "descriptionPreview:",
    ] {
        assert!(!sdl.contains(removed), "obsolete schema content {removed}");
    }
    assert!(
        sdl.contains("kind: String!"),
        "TasksEvent.kind stays dotted text"
    );
    assert!(sdl.contains("CLARIFICATION_REQUIRED"));
    assert!(!sdl.contains("REVIEW_READY"));
}

#[tokio::test]
async fn task_reads_require_an_authenticated_owner() {
    let schema = build_schema_without_request_principal(GraphqlState::for_tests());
    let response = schema
        .execute(
            r#"query {
              tasks(input: { workspaceId: "workspace:personal" }) {
                edges { node { taskId } }
              }
            }"#,
        )
        .await;
    assert_single_graphql_error(&response, "request is unauthenticated");
}

#[tokio::test]
async fn task_connections_reject_malformed_cursors_with_stable_code() {
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
    assert_error_code(&response, "invalid task cursor", "invalid_cursor");
}

#[tokio::test]
async fn task_connections_enforce_page_bounds_before_store_reads() {
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
        assert_error_code(&response, "invalid task cursor", "invalid_cursor");
    }
}

#[tokio::test]
async fn task_authorization_is_indistinguishable_before_identifier_validation() {
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
                client_id: None,
            }))
            .await;
        assert_error_code(&response, "task is unavailable", "task_unavailable");
    }
}

#[tokio::test]
async fn task_scope_filters_reject_semantic_stage_conflicts() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let response = schema
        .execute(
            r#"query {
              tasks(input: {
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
async fn task_mutations_require_client_idempotency_keys() {
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
async fn project_folder_and_task_executor_cwd_round_trip_through_graphql() {
    let store = crate::test_support::test_store().await;
    let agent = store.create_acp_agent("Fake ACP", "/bin/false", &[]).await.unwrap();
    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let project = schema.execute(r#"mutation {
      createProject(input: {
        workspaceId: "workspace:personal", name: "Code", folder: "/srv/code",
        clientMutationId: "acp-project"
      }) { project { projectId folder } }
    }"#).await;
    let project = response_json(project, "project json");
    let project_id = project["createProject"]["project"]["projectId"].as_str().unwrap();
    assert_eq!(project["createProject"]["project"]["folder"], "/srv/code");

    let captured = schema.execute(format!(r#"mutation {{
      captureTask(input: {{
        workspaceId: "workspace:personal", projectId: "{project_id}", title: "Use ACP",
        executorAgentId: "{}", cwdOverride: "/tmp/task-work",
        clientMutationId: "acp-capture"
      }}) {{
        task {{ executorAgentId executorBackend cwdOverride effectiveCwd effectiveCwdSource project {{ folder }} }}
      }}
    }}"#, agent.agent_id)).await;
    let captured = response_json(captured, "capture json");
    assert_json_values(&captured, &[
        ("/captureTask/task/executorAgentId", serde_json::json!(agent.agent_id)),
        ("/captureTask/task/executorBackend", serde_json::json!("acp")),
        ("/captureTask/task/cwdOverride", serde_json::json!("/tmp/task-work")),
        (
            "/captureTask/task/effectiveCwd",
            serde_json::json!("/tmp/task-work/use-acp"),
        ),
        ("/captureTask/task/effectiveCwdSource", serde_json::json!("task")),
        ("/captureTask/task/project/folder", serde_json::json!("/srv/code")),
    ]);

    let listed = schema
        .execute(
            r#"query {
              tasks(input: { workspaceId: "workspace:personal" }) {
                edges { node { effectiveCwd effectiveCwdSource } }
              }
            }"#,
        )
        .await;
    let listed = response_json(listed, "tasks json");
    assert_json_values(
        &listed,
        &[
            (
                "/tasks/edges/0/node/effectiveCwd",
                serde_json::json!("/tmp/task-work/use-acp"),
            ),
            (
                "/tasks/edges/0/node/effectiveCwdSource",
                serde_json::json!("task"),
            ),
        ],
    );
}

#[tokio::test]
async fn task_mutations_reject_whitespace_idempotency_aliases() {
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
async fn capture_task_returns_authoritative_task_projection() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
    let response = schema
        .execute(
            r#"mutation {
              captureTask(input: {
                workspaceId: "workspace:personal"
                title: "Capture through Tasks"
                taskDocument: "Exact request\n\nA durable capture.\n"
                schedule: {
                  scheduledFor: "2030-01-01T08:00:00Z"
                  timeZone: "UTC"
                  recurrence: { startsAt: "2030-01-01T08:00:00Z", cronExpression: "0 8 * * *" }
                }
                clientMutationId: "capture-work-graphql-test"
              }) {
                task {
                  taskId
                  title
                  stage { key behavior }
                  revision
                  generation
                  schedule { scheduledFor timeZone recurrenceId recurrenceRevision }
                  taskDocument
                  resultDocument
                  resultMetadata
                  reviewDocument
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
            ("/captureTask/task/revision", json!(1)),
            ("/captureTask/task/generation", json!(1)),
            ("/captureTask/task/schedule/scheduledFor", json!("2030-01-01T08:00:00Z")),
            ("/captureTask/task/schedule/timeZone", json!("UTC")),
            ("/captureTask/task/schedule/recurrenceRevision", json!(1)),
            (
                "/captureTask/task/taskDocument",
                json!("Exact request\n\nA durable capture.\n"),
            ),
            ("/captureTask/task/resultDocument", serde_json::Value::Null),
            ("/captureTask/task/resultMetadata", json!({})),
            ("/captureTask/task/reviewDocument", serde_json::Value::Null),
            ("/captureTask/task/artifacts", json!([])),
        ],
    );
    assert!(
        data["captureTask"]["eventCursor"]
            .as_str()
            .is_some_and(|cursor| !cursor.is_empty())
    );
    assert!(data["captureTask"]["task"]["schedule"]["recurrenceId"].as_str().is_some());
    let task_id = noema_tasks::TaskId::new(
        data["captureTask"]["task"]["taskId"]
            .as_str()
            .expect("Task id")
            .to_string(),
    )
    .expect("valid Task id");
    store
        .write_task_file(
            &task_id,
            noema_store::TASK_RESULT,
            "Current result.[^noema-source-1]\n\n[^noema-source-1]: [Current source](<https://example.com/current>)\n",
        )
        .await
        .expect("write current result");
    let current = response_json(
        schema
            .execute(format!(
                "query {{ task(taskId: \"{}\") {{ resultDocument resultMetadata }} }}",
                task_id.as_str()
            ))
            .await,
        "current Task result",
    );
    assert_eq!(current["task"]["resultDocument"], json!("Current result.\n\n"));
    assert_eq!(
        current["task"]["resultMetadata"],
        json!({
            "citations": [{
                "title": "Current source",
                "url": "https://example.com/current",
                "end_index": 15
            }]
        })
    );
}

#[tokio::test]
async fn task_workspace_lists_nested_files_and_rejects_unsafe_reads() {
    let store = crate::test_support::test_store().await;
    let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
    let captured = response_json(
        schema
            .execute(
                r#"mutation {
                  captureTask(input: {
                    workspaceId: "workspace:personal"
                    title: "Inspect workspace"
                    taskDocument: "Current Task"
                    clientMutationId: "capture-workspace-files"
                  }) { task { taskId } }
                }"#,
            )
            .await,
        "capture workspace Task",
    );
    let task_id = noema_tasks::TaskId::new(
        captured["captureTask"]["task"]["taskId"]
            .as_str()
            .expect("Task id")
            .to_string(),
    )
    .expect("valid Task id");
    store
        .write_task_file(&task_id, "notes/progress.md", "Nested progress")
        .await
        .expect("write support file");

    let workspace = response_json(
        schema
            .execute(format!(
                r#"query {{
                  task(taskId: "{}") {{
                    workspaceFiles {{ path isDirectory sizeBytes }}
                    workspaceFilesTruncated
                  }}
                  taskWorkspaceFile(taskId: "{}", path: "notes/progress.md") {{ path content }}
                }}"#,
                task_id.as_str(),
                task_id.as_str(),
            ))
            .await,
        "Task workspace",
    );
    let files = workspace["task"]["workspaceFiles"]
        .as_array()
        .expect("workspace files");
    assert!(files.iter().any(|file| file["path"] == "TASK.md"));
    assert!(files.iter().any(|file| file["path"] == "notes" && file["isDirectory"] == true));
    assert!(files.iter().any(|file| file["path"] == "notes/progress.md"));
    assert_eq!(workspace["task"]["workspaceFilesTruncated"], false);
    assert_eq!(workspace["taskWorkspaceFile"]["content"], "Nested progress");

    let unsafe_read = schema
        .execute(format!(
            r#"query {{ taskWorkspaceFile(taskId: "{}", path: "../outside.md") {{ path }} }}"#,
            task_id.as_str(),
        ))
        .await;
    assert_single_graphql_error(&unsafe_read, "task is unavailable");
}

#[tokio::test]
async fn task_gate_uses_the_unified_human_intervention_projection_until_resolved() {
    let store = crate::test_support::test_store().await;
    noema_store::test_support::initialize_codex_provider_selections(&store)
        .await
        .expect("initialize task providers");
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
    let queue = schema
        .execute(format!(
            r#"mutation {{
              queueTask(input: {{
                taskId: "{task_id}"
                expectedRevision: 1
                expectedGeneration: 1
                clientMutationId: "queue-intervention-projection"
              }}) {{ task {{ taskId }} }}
            }}"#
        ))
        .await;
    response_json(queue, "queued intervention task JSON");
    let service = noema_store::WorkCommandService::new(
        store,
        crate::test_support::ready_test_provider_registry(),
    );
    let claimed = service
        .claim_next_work_run("worker:intervention-projection", 60, &[])
        .await
        .expect("claim planner")
        .expect("queued planner");
    let fence = noema_store::WorkRunFence {
        run_id: claimed.run.run_id,
        lease_token: claimed.lease_token,
        task_generation: claimed.run.task_generation,
    };
    service
        .start_work_run(
            &fence,
            "actor:agent:test",
            None,
            "correlation:intervention:start",
        )
        .await
        .expect("start planner");
    let blocked = service
        .record_work_run_terminal(
            noema_store::WorkRunTerminal::Blocked(noema_store::ReportTaskBlocked {
                fence,
                prompt_markdown: "Which direction should the task take?".to_string(),
                context_markdown: "Choose the safest supported direction.".to_string(),
                suggested_answers: vec![
                    "Use the safer route".to_string(),
                    "Pause for now".to_string(),
                ],
                gate_kind: noema_tasks::TaskGateKind::Clarification,
            }),
            "actor:agent:test",
            None,
            "correlation:intervention:block",
        )
        .await
        .expect("open task gate");
    let waiting_task = blocked.task.expect("waiting task");
    let gate_id = blocked.gate_id.expect("open gate").to_string();
    let waiting_revision = waiting_task.revision;

    let pending = schema
        .execute(format!(
            r#"query {{
              pendingHumanInterventions(taskId: "{task_id}") {{
                __typename
                ... on TaskAttention {{
                  kind title summary validActions
                  gate {{ gateId kind prompt contextMarkdown suggestedAnswers }}
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
            ("/gate/gateId", json!(gate_id.clone())),
            ("/gate/prompt", json!("Which direction should the task take?")),
            (
                "/gate/suggestedAnswers",
                json!(["Use the safer route", "Pause for now"]),
            ),
            ("/task/taskId", json!(task_id)),
            ("/task/revision", json!(waiting_revision)),
            ("/task/generation", json!(1)),
        ],
    );
    assert_eq!(intervention["validActions"], json!(["ANSWER", "CANCEL"]));

    let answer = schema
        .execute(format!(
            r#"mutation {{
              answerTask(input: {{
                taskId: "{task_id}"
                expectedRevision: {waiting_revision}
                expectedGeneration: 1
                clientMutationId: "answer-intervention-projection"
                gateId: "{gate_id}"
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
        taskDocument: "Receipt-stable document"
        clientMutationId: "capture-receipt-stability"
      }) {
        task { taskId title taskDocument revision generation updatedAt }
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
                taskDocument: "Receipt-stable document"
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
