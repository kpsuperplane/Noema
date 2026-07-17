#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_store;

    #[tokio::test]
    async fn task_model_pools_query_projects_canonical_entries() {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated provider");
        let provider_registry = crate::test_support::ready_test_provider_registry();
        let entry = store
            .ensure_default_task_model_pool_settings_with_readiness(
                "codex",
                provider_registry.as_ref(),
            )
            .await
            .expect("task model settings")
            .into_iter()
            .find(|entry| entry.complexity == TaskComplexity::Simple)
            .expect("simple task model");

        let state = GraphqlState::for_tests_with_store(store);
        let entries = task_model_pools(&state, Some(GraphqlTaskComplexity::Simple))
            .await
            .expect("pool query");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].pool_entry_id, entry.pool_entry_id);
        assert_eq!(entries[0].model_profile, "gpt-5.6-luna");
    }

    #[tokio::test]
    async fn schema_exposes_three_global_settings_and_update_mutation() {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated provider");
        store
            .update_provider_account_metadata(
                "provider_account:codex:default",
                serde_json::json!({
                    "profiles": [{
                        "id": "gpt-5.6-luna",
                        "label": "GPT-5.6 Luna",
                        "reasoning_efforts": ["medium", "xhigh"],
                        "default_reasoning_effort": "medium"
                    }]
                }),
            )
            .await
            .expect("provider catalog");
        crate::test_support::initialize_codex_provider_selections(&store).await;
        let provider_registry = crate::test_support::ready_test_provider_registry();
        store
            .ensure_default_task_model_pool_settings_with_readiness(
                "codex",
                provider_registry.as_ref(),
            )
            .await
            .expect("task model settings");
        let schema = crate::graphql::build_schema(
            crate::graphql::GraphqlState::for_tests_with_store(store.clone()),
        );

        let update = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  updateTaskModelPoolEntry(poolEntryId: "task_pool:setting:simple", input: {
                    complexity: SIMPLE
                    label: "Fast"
                    providerKind: "codex"
                    providerAccountId: "provider_account:codex:default"
                    modelProfile: "gpt-5.6-luna"
                    reasoningEffort: MEDIUM
                    enabled: true
                    sortOrder: 0
                  }) {
                    poolEntryId
                    complexity
                    modelProfile
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("update mutation");
        let updated = update.data.into_json().expect("update json");
        assert_eq!(updated["updateTaskModelPoolEntry"]["complexity"], "SIMPLE");

        let query = schema
            .execute(async_graphql::Request::new(
                "{ taskModelPools { poolEntryId modelProfile } }",
            ))
            .await
            .into_result()
            .expect("pool query");
        let queried = query.data.into_json().expect("query json");
        assert_eq!(queried["taskModelPools"].as_array().map(Vec::len), Some(3));
        assert!(!schema.sdl().contains("createTaskModelPoolEntry"));
        assert!(!schema.sdl().contains("deleteTaskModelPoolEntry"));

        let invalid = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  updateTaskModelPoolEntry(poolEntryId: "task_pool:setting:simple", input: {
                    complexity: SIMPLE
                    providerKind: "codex"
                    providerAccountId: "provider_account:codex:default"
                    modelProfile: "model-that-does-not-exist"
                    reasoningEffort: MEDIUM
                    enabled: true
                    sortOrder: 0
                  }) { poolEntryId }
                }
                "#,
            ))
            .await;
        assert_eq!(invalid.errors.len(), 1);
        assert_eq!(
            invalid.errors[0].message,
            "model profile is not available for provider"
        );
    }

    #[tokio::test]
    async fn editing_and_disabling_stale_task_pool_route_need_no_provider_readiness() {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated provider");
        let provider_registry = crate::test_support::ready_test_provider_registry();
        let entry = store
            .ensure_default_task_model_pool_settings_with_readiness(
                "codex",
                provider_registry.as_ref(),
            )
            .await
            .expect("task model settings")
            .into_iter()
            .find(|entry| entry.complexity == TaskComplexity::Simple)
            .expect("simple task model");
        let pool_entry_id = entry.pool_entry_id.clone();
        let complexity: GraphqlTaskComplexity = entry.complexity.into();
        let provider_kind = entry.model.provider_kind;
        let provider_account_id = entry.model.provider_account_id;
        let model_profile = entry.model.model_profile.expect("model profile");
        let reasoning_effort = entry.model.reasoning_effort.map(Into::into);
        let sort_order = i32::try_from(entry.sort_order).expect("sort order");
        let state = GraphqlState::for_tests_with_store(store.clone())
            .with_provider_registry(std::sync::Arc::new(noema_providers::ProviderRegistry::new()));

        let edited = update_task_model_pool_entry(
            &state,
            "human:local",
            pool_entry_id.clone(),
            GraphqlTaskModelPoolEntryInput {
                complexity,
                label: Some("Unavailable but editable".to_string()),
                provider_kind: provider_kind.clone(),
                provider_account_id: provider_account_id.clone(),
                model_profile: model_profile.clone(),
                reasoning_effort,
                enabled: true,
                sort_order,
            },
        )
        .await
        .expect("edit stale route metadata");
        assert!(edited.enabled);
        assert_eq!(edited.label.as_deref(), Some("Unavailable but editable"));

        let updated = update_task_model_pool_entry(
            &state,
            "human:local",
            pool_entry_id.clone(),
            GraphqlTaskModelPoolEntryInput {
                complexity,
                label: edited.label,
                provider_kind,
                provider_account_id,
                model_profile,
                reasoning_effort,
                enabled: false,
                sort_order,
            },
        )
        .await
        .expect("disable stale route");

        assert!(!updated.enabled);
        assert!(
            !store
                .get_task_model_pool_entry(&pool_entry_id)
                .await
                .expect("pool entry")
                .expect("persisted entry")
                .enabled
        );
    }

    #[tokio::test]
    async fn task_execution_policy_is_global_and_mutable() {
        let store = test_store().await;
        let schema = crate::graphql::build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  updateTaskExecutionPolicy(input: {
                    maxProviderContinuations: 64
                    maxToolCalls: 256
                    maxActiveMinutes: 90
                    progressAuditInterval: 16
                  }) {
                    maxProviderContinuations
                    maxToolCalls
                    maxActiveMinutes
                    progressAuditInterval
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("update policy");
        let value = response.data.into_json().expect("policy json");
        assert_eq!(
            value["updateTaskExecutionPolicy"]["maxProviderContinuations"],
            64
        );

        let response = schema
            .execute(async_graphql::Request::new(
                "{ taskExecutionPolicy { maxToolCalls maxActiveMinutes } }",
            ))
            .await
            .into_result()
            .expect("query policy");
        let value = response.data.into_json().expect("policy json");
        assert_eq!(value["taskExecutionPolicy"]["maxToolCalls"], 256);
        assert_eq!(value["taskExecutionPolicy"]["maxActiveMinutes"], 90);
    }

    #[tokio::test]
    async fn resume_task_mutation_queues_a_linked_attempt() {
        let store = test_store().await;
        let (task, run) = crate::test_support::seed_task(&store, "Resume through GraphQL").await;
        let task_id = task.task_id;
        let leased = store
            .claim_next_agent_run("worker:test", "lease:test", 120)
            .await
            .expect("claim")
            .expect("leased run");
        assert_eq!(leased.run_id, run.run_id);
        store
            .transition_agent_run(
                &run.run_id,
                noema_tasks::RunStatus::Running,
                Some("lease:test"),
                None,
            )
            .await
            .expect("running run");
        for index in 1..=3 {
            store
                .append_agent_run_item(
                    noema_tasks::NewAgentRunItem {
                        item_id: Some(format!("run_item:page-{index}")),
                        run_id: run.run_id.clone(),
                        round_index: 0,
                        kind: noema_tasks::AgentRunItemKind::AssistantOutput,
                        status: noema_tasks::AgentRunItemStatus::Completed,
                        correlation_id: None,
                        parent_item_id: None,
                        content_text: Some(format!("item {index}")),
                        payload: serde_json::json!({"index": index}),
                    },
                    "lease:test",
                )
                .await
                .expect("run item");
        }
        store
            .transition_agent_run(
                &run.run_id,
                noema_tasks::RunStatus::Failed,
                Some("lease:test"),
                Some(("provider_error".to_string(), "model missing".to_string())),
            )
            .await
            .expect("failed run");
        let schema = crate::graphql::build_schema(GraphqlState::for_tests_with_store(store));

        let page = schema
            .execute(async_graphql::Request::new(format!(
                "{{ taskRunItems(runId: \"{}\", first: 2) {{ items {{ cursor contentText }} pageInfo {{ endCursor hasNextPage }} }} }}",
                run.run_id
            )))
            .await
            .into_result()
            .expect("task run items");
        let page = page.data.into_json().expect("page json");
        assert_eq!(page["taskRunItems"]["items"][0]["contentText"], "item 2");
        assert_eq!(page["taskRunItems"]["items"][1]["contentText"], "item 3");
        assert_eq!(page["taskRunItems"]["pageInfo"]["endCursor"], "2");
        assert_eq!(page["taskRunItems"]["pageInfo"]["hasNextPage"], true);

        let response = schema
            .execute(async_graphql::Request::new(format!(
                "mutation {{ resumeTask(taskId: \"{task_id}\") {{ taskId status latestRunId errorMessage runs {{ attemptIndex status }} }} }}"
            )))
            .await
            .into_result()
            .expect("resume mutation");
        let value = response.data.into_json().expect("resume json");

        assert_eq!(value["resumeTask"]["status"], "queued");
        assert_eq!(value["resumeTask"]["errorMessage"], serde_json::Value::Null);
        assert_eq!(
            value["resumeTask"]["runs"].as_array().map(Vec::len),
            Some(2)
        );
        assert_eq!(value["resumeTask"]["runs"][1]["attemptIndex"], 1);
    }

    #[tokio::test]
    async fn cancel_task_delivers_one_structured_conversation_event() {
        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated provider");
        crate::test_support::initialize_codex_provider_selections(&store).await;
        let provider_registry = crate::test_support::ready_test_provider_registry();
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let pool = store
            .ensure_default_task_model_pool_settings_with_readiness(
                "codex",
                provider_registry.as_ref(),
            )
            .await
            .expect("task models")
            .into_iter()
            .find(|entry| entry.complexity == TaskComplexity::Simple)
            .expect("simple model");
        let (task, _) = store
            .create_task_with_executor_with_readiness(
                noema_tasks::NewTask {
                    task_id: None,
                    title: "Cancellable task".to_string(),
                    request_markdown: "Stop when asked".to_string(),
                    complexity: TaskComplexity::Simple,
                    owner_human_id: "human:local".to_string(),
                    source: noema_tasks::TaskSource {
                        conversation_id: Some(conversation.conversation_id.clone()),
                        turn_id: None,
                        item_id: None,
                    },
                    created_by_agent_id: "agent:primary".to_string(),
                    creation_tool_call_id: None,
                    pool_entry_id: pool.pool_entry_id,
                    executor_model: pool.model.clone(),
                    reviewer_model: pool.model,
                    max_review_rounds: None,
                    criteria: vec![noema_tasks::NewTaskValidationCriterion {
                        criterion_id: None,
                        ordinal: 1,
                        description: "Stops".to_string(),
                        expected_evidence: None,
                    }],
                },
                provider_registry.as_ref(),
            )
            .await
            .expect("task");
        let schema =
            crate::graphql::build_schema(GraphqlState::for_tests_with_store(store.clone()));

        let response = schema
            .execute(async_graphql::Request::new(format!(
                "mutation {{ cancelTask(taskId: \"{}\") {{ status cancellable }} }}",
                task.task_id
            )))
            .await
            .into_result()
            .expect("cancel mutation");
        let value = response.data.into_json().expect("cancel json");
        assert_eq!(value["cancelTask"]["status"], "cancelled");
        assert_eq!(value["cancelTask"]["cancellable"], false);

        schema
            .execute(async_graphql::Request::new(format!(
                "mutation {{ cancelTask(taskId: \"{}\") {{ status }} }}",
                task.task_id
            )))
            .await
            .into_result()
            .expect("idempotent cancel mutation");

        let delivered = store
            .list_conversation_items(
                &conversation.conversation_id,
                noema_conversations::ReplayMode::Audit,
            )
            .await
            .expect("conversation items")
            .into_iter()
            .filter(|item| {
                item.kind == noema_conversations::ConversationItemKind::TaskReference
                    && item.metadata["source"] == "background_task_status"
            })
            .collect::<Vec<_>>();
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].payload_json["status"], "cancelled");
    }
}
