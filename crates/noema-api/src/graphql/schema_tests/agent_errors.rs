    #[tokio::test]
    async fn runtime_turn_passes_conversation_id_to_provider_request() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let (requests, runtime) =
            test_autofill_runtime_with_requests(store, "captured", None).await;
        let started = runtime
            .start_primary_conversation(None)
            .await
            .expect("conversation");
        let (item_tx, _item_rx) = tokio::sync::mpsc::unbounded_channel::<TurnStreamEvent>();

        runtime
            .turn_with_client_message_id(
                started.conversation_id.clone(),
                "hello from durable chat".to_string(),
                item_tx,
                None,
            )
            .await
            .expect("turn");

        let requests = requests.lock().expect("requests");
        assert!(
            requests.iter().any(|request| {
                request.conversation_id.as_deref() == Some(started.conversation_id.as_str())
                    && matches!(
                        &request.input,
                        noema_providers::GenerateInput::Messages(messages)
                            if messages.last().is_some_and(|message| {
                                message.role == noema_providers::GenerateMessageRole::User
                                    && message.content == "hello from durable chat"
                            })
                    )
            }),
            "captured requests: {requests:?}"
        );
    }
    #[tokio::test]
    async fn agents_query_sanitizes_unavailable_provider_errors() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let foundation = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .update_provider_account_status(
                &foundation.provider_account_id,
                noema_providers::ProviderAccountStatus::Unavailable,
                Some("bridge_missing"),
                Some("/Users/alice/.secret/token.txt failed with token abc123"),
            )
            .await
            .expect("status");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  agents {
                    modelOptions {
                      providerKind
                      disabledReason
                      profiles {
                        id
                        disabledReason
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let foundation_option = data["agents"][0]["modelOptions"]
            .as_array()
            .expect("options")
            .iter()
            .find(|option| option["providerKind"] == "foundation_local")
            .expect("foundation option");
        let disabled_reason = foundation_option["disabledReason"]
            .as_str()
            .expect("disabled reason");
        assert_eq!(
            disabled_reason,
            "Apple Foundation Models bridge is unavailable."
        );
        assert!(
            !serde_json::to_string(foundation_option)
                .expect("json text")
                .contains("/Users/alice")
        );
        assert!(
            !serde_json::to_string(foundation_option)
                .expect("json text")
                .contains("abc123")
        );
    }

    #[tokio::test]
    async fn agents_query_disables_unknown_foundation_local_provider() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  agents {
                    modelOptions {
                      providerKind
                      disabledReason
                      profiles {
                        id
                        disabledReason
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let foundation_option = data["agents"][0]["modelOptions"]
            .as_array()
            .expect("options")
            .iter()
            .find(|option| option["providerKind"] == "foundation_local")
            .expect("foundation option");
        assert_eq!(
            foundation_option["disabledReason"],
            "Apple Foundation Models availability has not been checked."
        );
        assert_eq!(
            foundation_option["profiles"][0]["disabledReason"],
            "Apple Foundation Models availability has not been checked."
        );
    }

    #[tokio::test]
    async fn save_agent_model_preference_rejects_unknown_foundation_local_provider() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let foundation = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveAgentModelPreference(input: {{
                    agentId: "agent:primary"
                    providerAccountId: "{}"
                    modelProfile: "default"
                  }}) {{
                    providerKind
                  }}
                }}
                "#,
                foundation.provider_account_id
            )))
            .await;

        assert!(
            response.errors.iter().any(|error| error.message
                == "Apple Foundation Models availability has not been checked."),
            "{:?}",
            response.errors
        );
    }

    #[tokio::test]
    async fn save_agent_model_preference_rejects_unavailable_provider() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let foundation = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .update_provider_account_status(
                &foundation.provider_account_id,
                noema_providers::ProviderAccountStatus::Unavailable,
                Some("unsupported_platform"),
                Some("/Users/alice/.secret/token.txt failed with token abc123"),
            )
            .await
            .expect("status");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveAgentModelPreference(input: {{
                    agentId: "agent:primary"
                    providerAccountId: "{}"
                    modelProfile: "default"
                  }}) {{
                    providerKind
                  }}
                }}
                "#,
                foundation.provider_account_id
            )))
            .await;

        assert_eq!(response.errors.len(), 1);
        let message = response.errors[0].message.as_str();
        assert_eq!(message, "Provider is unavailable on this platform.");
        assert!(!message.contains("/Users/alice"));
        assert!(!message.contains("abc123"));
        assert!(
            store
                .get_agent_runtime_preference("agent:primary")
                .await
                .expect("preference read")
                .is_none()
        );
    }
