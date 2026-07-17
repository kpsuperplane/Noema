    #[test]
    fn graphql_state_for_tests_has_runtime_state_accessors() {
        let state = GraphqlState::for_tests();
        assert!(state.optional_store().is_none());
        assert_eq!(
            state.memory_storage(),
            crate::graphql::local_status::GraphqlMemoryStorageStatus::Ready
        );
    }

    #[tokio::test]
    async fn provider_accounts_query_returns_safe_metadata() {
        use crate::test_support::test_store;
        use noema_providers::ProviderAccountStatus;

        let store = test_store().await;
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                &account.provider_account_id,
                ProviderAccountStatus::Authenticated,
                Some("codex_ok"),
                Some("Codex credentials are usable"),
            )
            .await
            .expect("status update");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  providerAccounts {
                    providerAccountId
                    providerKind
                    accountKey
                    displayName
                    authMethod
                    status
                    isActive
                    isDefault
                    lastCheckedAt
                    lastAuthenticatedAt
                    lastErrorCode
                    lastErrorMessage
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let account = &data["providerAccounts"][0];
        assert_eq!(
            account["providerAccountId"],
            "provider_account:codex:default"
        );
        assert_eq!(account["providerKind"], "codex");
        assert_eq!(account["accountKey"], "default");
        assert_eq!(account["displayName"], "Codex");
        assert_eq!(account["authMethod"], "oauth_device_code");
        assert_eq!(account["status"], "AUTHENTICATED");
        assert_eq!(account["isActive"], true);
        assert_eq!(account["isDefault"], true);
        assert_eq!(account["lastErrorCode"], "codex_ok");
        assert_eq!(account["lastErrorMessage"], "Codex credentials are usable");

        let json_text = serde_json::to_string(&data).expect("provider json");
        assert!(!json_text.contains("auth.json"));
        assert!(!json_text.contains("codex_tokens.json"));
        assert!(!json_text.contains("api_key"));
        assert!(!json_text.contains("token"));
    }

    #[tokio::test]
    async fn provider_account_catalog_lists_exa() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  providerAccountCatalog {
                    providerKind
                    displayName
                    authMethod
                    capabilities { capabilityId }
                  }
                  providerAccounts { providerKind }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(data["providerAccountCatalog"][0]["providerKind"], "exa");
        assert!(
            data["providerAccounts"]
                .as_array()
                .unwrap()
                .iter()
                .all(|account| { account["providerKind"] != "exa" })
        );
    }

    #[tokio::test]
    async fn create_exa_provider_account_stores_secret_without_returning_it() {
        use crate::test_support::test_store;

        let dir = tempfile::tempdir().expect("tempdir");
        let store = test_store().await;
        let state = GraphqlState::for_tests_with_store_and_environment(
            store,
            TestEnvironment::from_root(dir.path()).expect("paths"),
        );
        let schema = build_schema(state);

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  createProviderAccount(input: {
                    providerKind: "exa"
                    displayName: "Research"
                    secret: "secret-key"
                  }) {
                    providerAccountId
                    providerKind
                    accountKey
                    displayName
                    authMethod
                    status
                    lastErrorMessage
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let value = response.data.into_json().expect("json");
        let account = &value["createProviderAccount"];
        assert_eq!(account["providerKind"], "exa");
        assert_eq!(account["displayName"], "Research");
        assert_eq!(account["status"], "AUTHENTICATED");
        assert!(!value.to_string().contains("secret-key"));
        let account_key = account["accountKey"].as_str().expect("account key");
        assert!(
            dir.path()
                .join(format!("providers/exa/{account_key}/api_key.json"))
                .is_file()
        );
    }

    #[tokio::test]
    async fn provider_accounts_query_includes_created_exa_accounts() {
        use crate::test_support::test_store;

        let dir = tempfile::tempdir().expect("tempdir");
        let store = test_store().await;
        let state = GraphqlState::for_tests_with_store_and_environment(
            store,
            TestEnvironment::from_root(dir.path()).expect("paths"),
        );
        let schema = build_schema(state);

        let create_response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  createProviderAccount(input: {
                    providerKind: "exa"
                    displayName: "Research"
                    secret: "secret-key"
                  }) {
                    providerAccountId
                  }
                }
                "#,
            ))
            .await;
        assert!(
            create_response.errors.is_empty(),
            "{:?}",
            create_response.errors
        );

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  providerAccounts {
                    providerKind
                    displayName
                    authMethod
                    isDefault
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let accounts = response.data.into_json().expect("json")["providerAccounts"]
            .as_array()
            .expect("accounts")
            .clone();
        assert!(accounts.iter().any(|account| {
            account["providerKind"] == "exa"
                && account["displayName"] == "Research"
                && account["authMethod"] == "secret_input"
                && account["isDefault"] == false
        }));
    }

    #[tokio::test]
    async fn delete_provider_account_removes_exa_account_secret_and_binding() {
        use crate::test_support::test_store;

        let dir = tempfile::tempdir().expect("tempdir");
        let paths = TestEnvironment::from_root(dir.path()).expect("paths");
        let store = test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_environment(
            store.clone(),
            paths.clone(),
        ));

        let create_response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  createProviderAccount(input: {
                    providerKind: "exa"
                    displayName: "Research"
                    secret: "secret-key"
                  }) {
                    providerAccountId
                    accountKey
                  }
                }
                "#,
            ))
            .await;
        assert!(
            create_response.errors.is_empty(),
            "{:?}",
            create_response.errors
        );
        let create_data = create_response.data.into_json().expect("json");
        let created = &create_data["createProviderAccount"];
        let provider_account_id = created["providerAccountId"]
            .as_str()
            .expect("provider account id");
        let account_key = created["accountKey"].as_str().expect("account key");
        let account_home = paths.provider_account_home("exa", account_key);
        assert!(account_home.join("api_key.json").is_file());
        crate::test_support::save_provider_capability_assignment_for_tests(
            &store,
            "web.search",
            "web.search",
            noema_providers::ProviderCapabilityAccountReference::persisted(provider_account_id),
        )
        .await;

        let delete_response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  deleteProviderAccount(input: {{
                    providerAccountId: "{provider_account_id}"
                  }})
                }}
                "#
            )))
            .await;

        assert!(
            delete_response.errors.is_empty(),
            "{:?}",
            delete_response.errors
        );
        let delete_data = delete_response.data.into_json().expect("json");
        assert_eq!(delete_data["deleteProviderAccount"], true);
        assert!(
            store
                .get_provider_account(provider_account_id)
                .await
                .expect("load provider")
                .is_none()
        );
        assert!(
            store
                .provider_capability_binding("web.search", "web.search")
                .await
                .expect("load binding")
                .is_none()
        );
        assert!(!account_home.exists());
    }

    #[tokio::test]
    async fn delete_provider_account_rejects_default_accounts() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  deleteProviderAccount(input: {
                    providerAccountId: "provider_account:codex:default"
                  })
                }
                "#,
            ))
            .await;

        assert!(!response.errors.is_empty());
        assert!(
            response.errors[0]
                .message
                .contains("default provider accounts cannot be deleted")
        );
    }

    #[tokio::test]
    async fn provider_accounts_query_returns_all_active_default_accounts() {
        use crate::test_support::test_store;

        let store = test_store().await;
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
                  providerAccounts {
                    providerKind
                    accountKey
                    displayName
                    authMethod
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let accounts = response.data.into_json().expect("json")["providerAccounts"]
            .as_array()
            .expect("accounts")
            .clone();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0]["providerKind"], "codex");
        assert_eq!(accounts[1]["providerKind"], "foundation_local");
        assert_eq!(accounts[1]["authMethod"], "none");
    }
