#[tokio::test]
async fn agents_query_sanitizes_unavailable_provider_errors() {
    // Case: agents_query_sanitizes_unavailable_provider_errors.
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let local_models = store
            .ensure_default_local_models_provider_account()
            .await
            .expect("local models account");
        store
            .update_provider_account_status(
                &local_models.provider_account_id,
                noema_providers::ProviderAccountStatus::Unavailable,
                Some("unsupported_platform"),
                Some("/Users/alice/.secret/token.txt failed with token abc123"),
            )
            .await
            .expect("status");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"{
                  agents {
                    modelOptions {
                      providerKind disabledReason
                      profiles { id disabledReason }
                    }
                  }
                }"#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let local_models_option = data["agents"][0]["modelOptions"]
            .as_array()
            .expect("options")
            .iter()
            .find(|option| option["providerKind"] == "local_models")
            .expect("local models option");
        let disabled_reason = local_models_option["disabledReason"]
            .as_str()
            .expect("disabled reason");
        assert_eq!(disabled_reason, "Provider is unavailable on this platform.");
        assert!(
            !serde_json::to_string(local_models_option)
                .expect("json text")
                .contains("/Users/alice")
        );
        assert!(
            !serde_json::to_string(local_models_option)
                .expect("json text")
                .contains("abc123")
        );
    }
