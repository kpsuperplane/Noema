async fn delete_acp_agent(
    schema: &GraphqlSchema,
    agent_id: &str,
    expected_revision: u64,
) -> async_graphql::Response {
    schema
        .execute(format!(
            r#"mutation {{ deleteAcpAgent(input: {{
              agentId: "{agent_id}", expectedRevision: {expected_revision}
            }}) }}"#
        ))
        .await
}

#[tokio::test]
async fn delete_is_revision_fenced_and_removes_acp_setup_state() {
    let store = crate::test_support::test_store().await;
    let agent = store
        .create_acp_agent("Disposable ACP", "/bin/false", &[])
        .await
        .expect("create ACP agent");
    let authentication_attempt = store
        .begin_acp_auth_attempt(&agent.agent_id, agent.connection_revision, "login")
        .await
        .expect("begin ACP authentication");
    let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
    let stale = delete_acp_agent(&schema, &agent.agent_id, 2).await;
    assert_single_graphql_error(&stale, "settings changed");

    let authenticating = delete_acp_agent(&schema, &agent.agent_id, 1).await;
    assert_single_graphql_error(&authenticating, "authentication to finish");
    store
        .finish_acp_auth_attempt(&authentication_attempt, true, None)
        .await
        .expect("finish ACP authentication");

    let deleted = delete_acp_agent(&schema, &agent.agent_id, 1).await;
    assert!(deleted.errors.is_empty(), "{:?}", deleted.errors);
    assert_eq!(
        deleted.data.into_json().expect("delete response")["deleteAcpAgent"],
        true
    );
    assert!(store.get_acp_agent(&agent.agent_id).await.unwrap().is_none());
    assert!(store.get_agent(&agent.agent_id).await.unwrap().is_none());
}

#[tokio::test]
async fn delete_rejects_current_task_and_schedule_references() {
    let store = crate::test_support::test_store().await;
    let agent = store
        .create_acp_agent("Assigned ACP", "/bin/false", &[])
        .await
        .expect("create ACP agent");
    let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
    let captured = schema
        .execute(format!(
            r#"mutation {{ captureTask(input: {{
              workspaceId: "workspace:personal"
              title: "Keep assigned executor"
              executorAgentId: "{}"
              schedule: {{
                scheduledFor: "2030-01-01T08:00:00Z"
                timeZone: "UTC"
                recurrence: {{
                  startsAt: "2030-01-01T08:00:00Z"
                  cronExpression: "0 8 * * *"
                }}
              }}
              clientMutationId: "capture-acp-delete-guard"
            }}) {{ task {{ taskId }} }} }}"#,
            agent.agent_id
        ))
        .await;
    assert!(captured.errors.is_empty(), "{:?}", captured.errors);
    let captured = captured.data.into_json().expect("capture response");
    let task_id = captured["captureTask"]["task"]["taskId"]
        .as_str()
        .expect("captured task id");

    let current_task = delete_acp_agent(&schema, &agent.agent_id, 1).await;
    assert_single_graphql_error(&current_task, "Reassign or cancel");

    let cancelled = schema
        .execute(format!(
            r#"mutation {{ cancelTask(input: {{
              taskId: "{task_id}"
              expectedRevision: 1
              expectedGeneration: 1
              clientMutationId: "cancel-acp-delete-guard"
            }}) {{ task {{ taskId }} }} }}"#
        ))
        .await;
    assert!(cancelled.errors.is_empty(), "{:?}", cancelled.errors);

    let active_schedule = delete_acp_agent(&schema, &agent.agent_id, 1).await;
    assert_single_graphql_error(&active_schedule, "end recurring schedules");
    assert!(store.get_acp_agent(&agent.agent_id).await.unwrap().is_some());
}
