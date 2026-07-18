    async fn local_conversation_subscription_fixture(
    ) -> (GraphqlState, RuntimeEventRegistry, String) {
        let store = crate::test_support::test_store().await;
        let conversation = store
            .create_conversation(noema_conversations::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let state = GraphqlState::for_tests_with_store(store);
        let subscriptions = state.subscriptions().clone();
        (state, subscriptions, conversation.conversation_id)
    }

    #[tokio::test]
    async fn conversation_events_emits_ready_before_live_events() {
        let (state, subscriptions, conversation_id) =
            local_conversation_subscription_fixture().await;
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(format!(
            r#"subscription {{
              conversationEvents(conversationId: "{}") {{
                __typename
                ... on SubscriptionReadyEvent {{ conversationId }}
                ... on TurnCompletedEvent {{ conversationId clientMessageId }}
              }}
            }}"#,
            conversation_id,
        )));

        let response = stream.next().await.expect("ready response");
        let data = response.data.into_json().expect("ready json");
        assert_json_fields!(data,
            "/conversationEvents/__typename" => "SubscriptionReadyEvent",
            "/conversationEvents/conversationId" => conversation_id,
        );

        for index in 0..256 {
            subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
                client_message_id: None,
                event: Box::new(TurnStreamEvent::ConversationItem {
                    conversation_id: conversation_id.clone(),
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
            conversation_id: conversation_id.clone(),
            client_message_id: Some("client_1".to_string()),
        });
        let response = stream.next().await.expect("resynchronization response");
        let data = response.data.into_json().expect("resynchronization json");
        assert_json_fields!(data, "/conversationEvents/__typename" => "SubscriptionReadyEvent");
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
        assert_json_fields!(data,
            "/conversationEvents/__typename" => "TurnCompletedEvent",
            "/conversationEvents/clientMessageId" => "client_1",
        );
    }

    #[tokio::test]
    async fn task_events_backfills_from_durable_cursor() {
        let store = crate::test_support::test_store().await;
        let (task, run) = crate::test_support::seed_task(&store, "Subscription task").await;
        let state = GraphqlState::for_tests_with_store(store.clone());
        let runtime_events = state.subscriptions().clone();
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(format!(
            r#"subscription {{
              taskEvents(taskId: "{}", after: "0") {{
                cursor kind taskId status
                run {{ runId providerCallCount }}
              }}
            }}"#,
            task.task_id
        )));
        let response = stream.next().await.expect("task event response");
        let data = response.data.into_json().expect("task event json");
        assert_json_fields!(data,
            "/taskEvents/taskId" => task.task_id,
            "/taskEvents/cursor" => "1",
            "/taskEvents/kind" => "TASK_UPDATED",
            "/taskEvents/run/runId" => run.run_id,
            "/taskEvents/run/providerCallCount" => 0,
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
        assert_json_fields!(data,
            "/taskEvents/cursor" => "2",
            "/taskEvents/kind" => "TASK_UPDATED",
            "/taskEvents/status" => "queued",
        );
    }

    #[tokio::test]
    async fn subscription_projects_live_delta_and_conversation_item_events() {
        let (state, subscriptions, conversation_id) =
            local_conversation_subscription_fixture().await;
        let schema = build_schema(state);
        let mut stream = schema.execute_stream(async_graphql::Request::new(format!(
            r#"subscription {{
              conversationEvents(conversationId: "{}") {{
                __typename
                ... on AssistantTextDeltaEvent {{ conversationId turnId streamId delta }}
                ... on ConversationItemEvent {{ conversationId itemId cursor metadata }}
              }}
            }}"#,
            conversation_id,
        )));

        let ready = stream.next().await.expect("ready response");
        assert_eq!(
            ready.data.into_json().expect("ready json")["conversationEvents"]["__typename"],
            "SubscriptionReadyEvent"
        );

        subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::AssistantTextDelta {
                conversation_id: conversation_id.clone(),
                turn_id: "turn_1".to_string(),
                stream_id: "assistant_stream:turn_1:initial".to_string(),
                response_index: 0,
                delta: "Hel".to_string(),
            }),
        });

        let response = stream.next().await.expect("delta response");
        let data = response.data.into_json().expect("delta json");
        let event = &data["conversationEvents"];
        assert_json_fields!(event,
            "/__typename" => "AssistantTextDeltaEvent",
            "/conversationId" => conversation_id,
            "/turnId" => "turn_1",
            "/streamId" => "assistant_stream:turn_1:initial",
            "/delta" => "Hel",
        );

        subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::ConversationItem {
                conversation_id: conversation_id.clone(),
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
        assert_json_fields!(event,
            "/__typename" => "ConversationItemEvent",
            "/conversationId" => conversation_id,
            "/itemId" => "transient:activity_1",
        );
        assert!(event["cursor"].is_null());
        assert_eq!(
            event["metadata"],
            json!({
                "runtime_item_id": "activity_1",
                "transient": true,
            })
        );

        subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
            client_message_id: None,
            event: Box::new(TurnStreamEvent::ConversationItem {
                conversation_id: conversation_id.clone(),
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
        assert_json_fields!(event,
            "/__typename" => "ConversationItemEvent",
            "/conversationId" => conversation_id,
            "/itemId" => "item_1",
            "/cursor" => "conversation_item:1",
            "/metadata" => json!({}),
        );
    }

    #[tokio::test]
    async fn conversation_events_rejects_foreign_human_conversations() {
        let store = crate::test_support::test_store().await;
        let conversation = store
            .create_conversation(conversation_for_human("human:other"))
            .await
            .expect("foreign conversation");
        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let mut stream = schema.execute_stream(async_graphql::Request::new(format!(
            r#"subscription {{
              conversationEvents(conversationId: "{}") {{ __typename }}
            }}"#,
            conversation.conversation_id,
        )));

        let response = stream.next().await.expect("subscription response");
        assert_single_graphql_error(&response, "conversation is unavailable");
    }
