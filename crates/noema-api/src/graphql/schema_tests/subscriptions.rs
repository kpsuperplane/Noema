    #[tokio::test]
    async fn conversation_events_emits_ready_before_live_events() {
        let state = GraphqlState::for_tests();
        let subscriptions = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"
            subscription {
              conversationEvents(conversationId: "conversation_1") {
                __typename
                ... on SubscriptionReadyEvent {
                  conversationId
                }
                ... on TurnCompletedEvent {
                  conversationId
                  clientMessageId
                }
              }
            }
            "#,
        ));

        let response = stream.next().await.expect("ready response");
        let data = response.data.into_json().expect("ready json");
        assert_eq!(
            data.pointer("/conversationEvents/__typename")
                .and_then(serde_json::Value::as_str),
            Some("SubscriptionReadyEvent")
        );
        assert_eq!(
            data.pointer("/conversationEvents/conversationId")
                .and_then(serde_json::Value::as_str),
            Some("conversation_1")
        );

        for index in 0..256 {
            subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
                client_message_id: None,
                event: Box::new(TurnStreamEvent::ConversationItem {
                    conversation_id: "conversation_1".to_string(),
                    item_id: format!("item_{index}"),
                    cursor: Some(format!("conversation_item:{}", index + 1)),
                    turn_id: Some("turn_1".to_string()),
                    metadata: serde_json::json!({}),
                    item: Box::new(noema_runtime::TurnTranscriptItem::UserText {
                        text: format!("message {index}"),
                    }),
                }),
            });
        }
        subscriptions.publish_conversation(ConversationRuntimeEvent::Completed {
            conversation_id: "conversation_1".to_string(),
            client_message_id: Some("client_1".to_string()),
        });
        let response = stream.next().await.expect("resynchronization response");
        let data = response.data.into_json().expect("resynchronization json");
        assert_eq!(
            data.pointer("/conversationEvents/__typename")
                .and_then(serde_json::Value::as_str),
            Some("SubscriptionReadyEvent")
        );
        let data = loop {
            let response = stream.next().await.expect("live response after lag");
            let data = response.data.into_json().expect("live response json");
            if data
                .pointer("/conversationEvents/__typename")
                .and_then(serde_json::Value::as_str)
                == Some("TurnCompletedEvent")
            {
                break data;
            }
        };
        assert_eq!(
            data.pointer("/conversationEvents/__typename")
                .and_then(serde_json::Value::as_str),
            Some("TurnCompletedEvent")
        );
        assert_eq!(
            data.pointer("/conversationEvents/clientMessageId")
                .and_then(serde_json::Value::as_str),
            Some("client_1")
        );
    }

    #[tokio::test]
    async fn task_events_backfills_from_durable_cursor() {
        let store = crate::test_support::test_store().await;
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
        let pool = store
            .ensure_default_task_model_pool_settings_with_readiness(
                "codex",
                provider_registry.as_ref(),
            )
            .await
            .expect("task models")
            .into_iter()
            .find(|entry| entry.complexity == noema_tasks::TaskComplexity::Simple)
            .expect("simple task model");
        let (task, run) = store
            .create_task_with_executor_with_readiness(
                noema_tasks::NewTask {
                    task_id: None,
                    title: "Subscription task".to_string(),
                    request_markdown: "Stream updates".to_string(),
                    complexity: noema_tasks::TaskComplexity::Simple,
                    owner_human_id: "human:local".to_string(),
                    source: noema_tasks::TaskSource::default(),
                    created_by_agent_id: "agent:primary".to_string(),
                    creation_tool_call_id: None,
                    pool_entry_id: pool.pool_entry_id,
                    executor_model: pool.model.clone(),
                    reviewer_model: pool.model,
                    max_review_rounds: None,
                    criteria: vec![noema_tasks::NewTaskValidationCriterion {
                        criterion_id: None,
                        ordinal: 1,
                        description: "Completes".to_string(),
                        expected_evidence: None,
                    }],
                },
                provider_registry.as_ref(),
            )
            .await
            .expect("task");
        let state = GraphqlState::for_tests_with_store(store.clone());
        let runtime_events = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(format!(
            r#"
            subscription {{
              taskEvents(taskId: "{}", after: "0") {{
                cursor
                kind
                taskId
                status
                run {{
                  runId
                  providerCallCount
                }}
              }}
            }}
            "#,
            task.task_id
        )));
        let response = stream.next().await.expect("task event response");
        let data = response.data.into_json().expect("task event json");
        assert_eq!(
            data.pointer("/taskEvents/taskId")
                .and_then(serde_json::Value::as_str),
            Some(task.task_id.as_str())
        );
        assert_eq!(
            data.pointer("/taskEvents/cursor")
                .and_then(serde_json::Value::as_str),
            Some("1")
        );
        assert_eq!(
            data.pointer("/taskEvents/kind")
                .and_then(serde_json::Value::as_str),
            Some("TASK_UPDATED")
        );
        assert_eq!(
            data.pointer("/taskEvents/run/runId")
                .and_then(serde_json::Value::as_str),
            Some(run.run_id.as_str())
        );
        assert_eq!(
            data.pointer("/taskEvents/run/providerCallCount")
                .and_then(serde_json::Value::as_i64),
            Some(0)
        );

        store
            .append_task_event(noema_tasks::NewTaskEvent {
                event_id: None,
                task_id: task.task_id.clone(),
                event_kind: noema_tasks::TaskEventKind::new("task.live_test")
                    .expect("valid event kind"),
                actor_id: "runtime:test".to_string(),
                causation_id: None,
                correlation_id: None,
                payload: serde_json::json!({}),
            })
            .await
            .expect("append live task event");
        runtime_events.publish_task(TaskRuntimeEvent::Changed {
            task_id: task.task_id.clone(),
        });

        let response = stream.next().await.expect("live task event response");
        let data = response.data.into_json().expect("live task event json");
        assert_eq!(
            data.pointer("/taskEvents/cursor")
                .and_then(serde_json::Value::as_str),
            Some("2")
        );
        assert_eq!(
            data.pointer("/taskEvents/kind")
                .and_then(serde_json::Value::as_str),
            Some("TASK_UPDATED")
        );
        assert_eq!(
            data.pointer("/taskEvents/status")
                .and_then(serde_json::Value::as_str),
            Some("queued")
        );
    }

    #[tokio::test]
    async fn subscription_streams_assistant_text_delta_event() {
        let state = GraphqlState::for_tests();
        let subscriptions = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"
            subscription {
              conversationEvents(conversationId: "conversation_1") {
                __typename
                ... on AssistantTextDeltaEvent {
                  conversationId
                  turnId
                  streamId
                  delta
                }
              }
            }
            "#,
        ));

        let ready = stream.next().await.expect("ready response");
        assert_eq!(
            ready.data.into_json().expect("ready json")["conversationEvents"]["__typename"],
            "SubscriptionReadyEvent"
        );

        subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::AssistantTextDelta {
                conversation_id: "conversation_1".to_string(),
                turn_id: "turn_1".to_string(),
                stream_id: "assistant_stream:turn_1:initial".to_string(),
                response_index: 0,
                delta: "Hel".to_string(),
            }),
        });

        let response = stream.next().await.expect("delta response");
        let data = response.data.into_json().expect("delta json");
        let event = &data["conversationEvents"];
        assert_eq!(event["__typename"], "AssistantTextDeltaEvent");
        assert_eq!(event["conversationId"], "conversation_1");
        assert_eq!(event["turnId"], "turn_1");
        assert_eq!(event["streamId"], "assistant_stream:turn_1:initial");
        assert_eq!(event["delta"], "Hel");
    }

    #[tokio::test]
    async fn subscription_streams_conversation_item_metadata() {
        let state = GraphqlState::for_tests();
        let subscriptions = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"
            subscription {
              conversationEvents(conversationId: "conversation_1") {
                __typename
                ... on ConversationItemEvent {
                  conversationId
                  itemId
                  cursor
                  metadata
                }
              }
            }
            "#,
        ));

        let ready = stream.next().await.expect("ready response");
        assert_eq!(
            ready.data.into_json().expect("ready json")["conversationEvents"]["__typename"],
            "SubscriptionReadyEvent"
        );

        subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::ConversationItem {
                conversation_id: "conversation_1".to_string(),
                item_id: "transient:activity_1".to_string(),
                cursor: None,
                turn_id: Some("turn_1".to_string()),
                metadata: json!({
                    "runtime_item_id": "activity_1",
                    "transient": true,
                }),
                item: Box::new(noema_runtime::TurnTranscriptItem::AssistantText {
                    text: "Hello".to_string(),
                }),
            }),
        });

        let response = stream.next().await.expect("item response");
        let data = response.data.into_json().expect("item json");
        let event = &data["conversationEvents"];
        assert_eq!(event["__typename"], "ConversationItemEvent");
        assert_eq!(event["conversationId"], "conversation_1");
        assert_eq!(event["itemId"], "transient:activity_1");
        assert!(event["cursor"].is_null());
        assert_eq!(
            event["metadata"],
            json!({
                "runtime_item_id": "activity_1",
                "transient": true,
            })
        );
    }

    #[tokio::test]
    async fn subscription_streams_conversation_item_cursor_when_present() {
        let state = GraphqlState::for_tests();
        let subscriptions = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"
            subscription {
              conversationEvents(conversationId: "conversation_1") {
                __typename
                ... on ConversationItemEvent {
                  itemId
                  cursor
                }
              }
            }
            "#,
        ));

        let ready = stream.next().await.expect("ready response");
        assert_eq!(
            ready.data.into_json().expect("ready json")["conversationEvents"]["__typename"],
            "SubscriptionReadyEvent"
        );

        subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::ConversationItem {
                conversation_id: "conversation_1".to_string(),
                item_id: "item_1".to_string(),
                cursor: Some("conversation_item:1".to_string()),
                turn_id: Some("turn_1".to_string()),
                metadata: serde_json::json!({}),
                item: Box::new(noema_runtime::TurnTranscriptItem::UserText {
                    text: "Hello".to_string(),
                }),
            }),
        });

        let response = stream.next().await.expect("item response");
        let data = response.data.into_json().expect("item json");
        let event = &data["conversationEvents"];
        assert_eq!(event["__typename"], "ConversationItemEvent");
        assert_eq!(event["itemId"], "item_1");
        assert_eq!(event["cursor"], "conversation_item:1");
    }
