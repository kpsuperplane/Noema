    #[tokio::test]
    async fn agent_preference_enforces_identity_and_reasoning_contracts() {
        let (schema, account_id) = schema_with_reasoning_profile().await;

        let options = schema
            .execute(
                "{ agents { modelOptions { profiles { id reasoningEfforts defaultReasoningEffort } } } }",
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
                    modelProfile: "gpt-5.5"
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
                    modelProfile: "gpt-5.5", reasoningEffort: HIGH
                  }}) {{ modelProfile reasoningEffort }}
                }}"#,
            ))
            .await;
        assert!(valid.errors.is_empty(), "{:?}", valid.errors);
        let data = valid.data.into_json().expect("json");
        assert_eq!(data["saveAgentModelPreference"]["reasoningEffort"], "HIGH");

        let task_executor = schema
            .execute(format!(
                r#"mutation {{
                  saveAgentModelPreference(input: {{
                    agentId: "agent:task-executor", providerAccountId: "{account_id}",
                    modelProfile: "gpt-5.5", reasoningEffort: MEDIUM
                  }}) {{ modelProfile }}
                }}"#,
            ))
            .await;
        assert_single_graphql_error(&task_executor, "complexity tier");
    }
