    #[tokio::test]
    async fn agent_preference_enforces_identity_and_reasoning_contracts() {
        let (schema, account_id) = schema_with_reasoning_profile().await;

        let options = schema
            .execute(
                "{ agents { modelOptions { profiles { id reasoningEfforts defaultReasoningEffort } recommendations { useCase modelProfile reasoningEffort disabledReason } } } }",
            )
            .await;
        assert!(options.errors.is_empty(), "{:?}", options.errors);
        let options = options.data.into_json().expect("options json");
        let profile = &options["agents"][0]["modelOptions"][0]["profiles"][0];
        assert_eq!(
            profile["reasoningEfforts"],
            serde_json::json!(["LOW", "MEDIUM", "HIGH"])
        );
        assert_eq!(profile["defaultReasoningEffort"], "MEDIUM");

        let missing_reasoning = schema
            .execute(format!(
                r#"mutation {{
                  saveAgentModelPreference(input: {{
                    agentId: "agent:primary", providerAccountId: "{account_id}",
                    selectionMode: EXPLICIT_PROFILE, modelProfile: "gpt-5.5"
                  }}) {{ modelProfile }}
                }}"#,
            ))
            .await;
        assert_single_graphql_error(&missing_reasoning, "reasoning");

        let valid = schema
            .execute(format!(
                r#"mutation {{
                  saveAgentModelPreference(input: {{
                    agentId: "agent:primary", providerAccountId: "{account_id}",
                    selectionMode: EXPLICIT_PROFILE,
                    modelProfile: "gpt-5.5", reasoningEffort: HIGH
                  }}) {{ modelProfile reasoningEffort selectionMode }}
                }}"#,
            ))
            .await;
        assert!(valid.errors.is_empty(), "{:?}", valid.errors);
        let data = valid.data.into_json().expect("json");
        assert_eq!(data["saveAgentModelPreference"]["reasoningEffort"], "HIGH");
        assert_eq!(
            data["saveAgentModelPreference"]["selectionMode"],
            "EXPLICIT_PROFILE"
        );

        let recommended = schema
            .execute(format!(
                r#"mutation {{
                  saveAgentModelPreference(input: {{
                    agentId: "agent:primary", providerAccountId: "{account_id}",
                    selectionMode: NOEMA_RECOMMENDED
                  }}) {{ modelProfile reasoningEffort selectionMode }}
                }}"#,
            ))
            .await;
        assert!(recommended.errors.is_empty(), "{:?}", recommended.errors);
        assert_eq!(
            recommended.data.into_json().expect("json")["saveAgentModelPreference"],
            serde_json::json!({
                "modelProfile": null,
                "reasoningEffort": null,
                "selectionMode": "NOEMA_RECOMMENDED"
            })
        );

        let invalid_recommended = schema
            .execute(format!(
                r#"mutation {{
                  saveAgentModelPreference(input: {{
                    agentId: "agent:primary", providerAccountId: "{account_id}",
                    selectionMode: NOEMA_RECOMMENDED, modelProfile: "gpt-5.5"
                  }}) {{ selectionMode }}
                }}"#,
            ))
            .await;
        assert_single_graphql_error(&invalid_recommended, "does not accept");

        let task_executor = schema
            .execute(format!(
                r#"mutation {{
                  saveAgentModelPreference(input: {{
                    agentId: "agent:task-executor", providerAccountId: "{account_id}",
                    selectionMode: EXPLICIT_PROFILE,
                    modelProfile: "gpt-5.5", reasoningEffort: MEDIUM
                  }}) {{ modelProfile }}
                }}"#,
            ))
            .await;
        assert_single_graphql_error(&task_executor, "complexity tier");
    }

    #[tokio::test]
    async fn recommended_preference_can_use_provider_default_reasoning() {
        use noema_providers::{
            NewProviderAccount, ProviderAccountStatus, ProviderAuthMethod,
            provider_account_from_persisted,
        };

        let store = crate::test_support::test_store().await;
        let account = store
            .create_provider_account(NewProviderAccount {
                provider_kind: "openrouter".to_string(),
                display_name: None,
                auth_method: ProviderAuthMethod::OauthPkce,
                status: ProviderAccountStatus::Authenticated,
                metadata: serde_json::json!({
                    "profiles": [{
                        "id": "deepseek/deepseek-v4-flash",
                        "label": "DeepSeek V4 Flash",
                        "reasoning_efforts": ["low", "medium"]
                    }]
                }),
            })
            .await
            .expect("OpenRouter account");
        let account = provider_account_from_persisted(account);

        let (_, model_profile, reasoning_effort) =
            crate::graphql::agents::resolve_preference_input(
                &store,
                &account,
                crate::graphql::agents::GraphqlModelPreferenceSelectionMode::NoemaRecommended,
                None,
                None,
                noema_providers::NoemaModelUseCase::ActionReviewer,
            )
            .await
            .expect("recommended selection");

        assert_eq!(model_profile, "deepseek/deepseek-v4-flash");
        assert_eq!(reasoning_effort, None);
    }
