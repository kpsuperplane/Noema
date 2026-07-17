    #[tokio::test]
    async fn agents_query_returns_safe_agent_metadata() {
        use noema_store::NewAgent;

        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("default actors");
        store
            .update_agent_display_name("agent:primary", "Noema")
            .await
            .expect("name primary");
        store
            .create_agent(NewAgent {
                agent_id: "agent:unnamed".to_string(),
                display_name: None,
            })
            .await
            .expect("unnamed agent");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  agents {
                    agentId
                    displayName
                    isPrimary
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let agents = data["agents"].as_array().expect("agents array");
        let primary = agents
            .iter()
            .find(|agent| agent["agentId"] == "agent:primary")
            .expect("primary agent");
        assert_eq!(primary["displayName"], "Noema");
        assert_eq!(primary["isPrimary"], true);
        let unnamed = agents
            .iter()
            .find(|agent| agent["agentId"] == "agent:unnamed")
            .expect("unnamed agent");
        assert_eq!(unnamed["displayName"], serde_json::Value::Null);
        assert_eq!(unnamed["isPrimary"], false);

        let json_text = serde_json::to_string(&data).expect("agent json");
        assert!(!json_text.contains("prompt"));
        assert!(!json_text.contains("memory"));
        assert!(!json_text.contains("runtime"));
        assert!(!json_text.contains("credential"));
        assert!(!json_text.contains("conversation"));
    }

    #[tokio::test]
    async fn agents_query_exposes_model_preference_options() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_metadata(
                &codex.provider_account_id,
                serde_json::json!({
                    "profiles": [
                        { "id": "gpt-5.5", "label": "GPT-5.5" },
                        { "id": "gpt-5.4", "label": "GPT-5.4" },
                        { "id": "gpt-5.4-mini", "label": "GPT-5.4-Mini" },
                        { "id": "gpt-5.3-codex-spark", "label": "GPT-5.3-Codex-Spark" }
                    ],
                    "models_source": "codex_models_endpoint"
                }),
            )
            .await
            .expect("codex metadata");
        let foundation = store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");
        store
            .update_provider_account_status(
                &foundation.provider_account_id,
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated foundation account");
        let ready_selection = crate::test_support::ready_provider_selection(
            noema_providers::ProviderSelectionSnapshot::explicit(
                "foundation_local",
                &foundation.provider_account_id,
                "default",
                None,
                Some("agent_preference_test".to_string()),
            ),
        );
        store
            .upsert_agent_runtime_preference_with_ready_selection(
                noema_store::NewAgentRuntimePreference {
                    agent_id: "agent:primary".to_string(),
                    provider_kind: "foundation_local".to_string(),
                    provider_account_id: foundation.provider_account_id,
                    model_profile: "default".to_string(),
                    reasoning_effort: None,
                },
                &ready_selection,
            )
            .await
            .expect("preference");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  agents {
                    agentId
                    modelPreference {
                      providerKind
                      providerAccountId
                      modelProfile
                    }
                    modelOptions {
                      providerKind
                      providerAccountId
                      providerDisplayName
                      status
                      profiles {
                        id
                        label
                        disabledReason
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let agent = &response.data.into_json().expect("json")["agents"][0];
        assert_eq!(agent["modelPreference"]["providerKind"], "foundation_local");
        assert_eq!(agent["modelPreference"]["modelProfile"], "default");
        assert_eq!(agent["modelOptions"][0]["providerKind"], "codex");
        let codex_profile_ids: Vec<_> = agent["modelOptions"][0]["profiles"]
            .as_array()
            .expect("codex profiles")
            .iter()
            .map(|profile| profile["id"].as_str().expect("profile id"))
            .collect();
        assert_eq!(
            codex_profile_ids,
            ["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex-spark"]
        );
        assert_eq!(agent["modelOptions"][1]["providerKind"], "foundation_local");
        assert_eq!(
            agent["modelOptions"][1]["profiles"][0]["label"],
            "Default on-device"
        );
    }

    #[tokio::test]
    async fn agents_query_exposes_profile_reasoning_efforts() {
        let (schema, _) = schema_with_reasoning_profile().await;
        let response = schema
            .execute(
                r#"
              query {
                agents {
                  modelOptions {
                    profiles {
                      id
                      reasoningEfforts
                      defaultReasoningEffort
                    }
                  }
                }
              }
            "#,
            )
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let profile = &data["agents"][0]["modelOptions"][0]["profiles"][0];
        assert_eq!(
            profile["reasoningEfforts"],
            serde_json::json!(["LOW", "MEDIUM", "HIGH"])
        );
        assert_eq!(profile["defaultReasoningEffort"], "MEDIUM");
    }

    #[tokio::test]
    async fn codex_profile_metadata_reasoning_efforts_are_exposed() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_status(
                &account.provider_account_id,
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("status");
        store
            .update_provider_account_metadata(
                &account.provider_account_id,
                serde_json::json!({
                    "profiles": [{
                        "id": "gpt-5.5",
                        "label": "GPT-5.5",
                        "reasoning_efforts": ["low", "medium", "high"],
                        "default_reasoning_effort": "medium"
                    }]
                }),
            )
            .await
            .expect("metadata");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(
                r#"
              query {
                agents {
                  modelOptions {
                    providerKind
                    providerAccountId
                    profiles {
                      id
                      reasoningEfforts
                      defaultReasoningEffort
                    }
                  }
                }
              }
            "#,
            )
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let codex_option = data["agents"][0]["modelOptions"]
            .as_array()
            .expect("model options")
            .iter()
            .find(|option| option["providerKind"] == "codex")
            .expect("codex option");
        let profile = &codex_option["profiles"][0];
        assert_eq!(
            profile["reasoningEfforts"],
            serde_json::json!(["LOW", "MEDIUM", "HIGH"])
        );
        assert_eq!(profile["defaultReasoningEffort"], "MEDIUM");

        let account_id = codex_option["providerAccountId"]
            .as_str()
            .expect("provider account id");
        let save_without_reasoning = schema
            .execute(format!(
                r#"
            mutation {{
              saveAgentModelPreference(input: {{
                agentId: "agent:primary",
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5"
              }}) {{
                modelProfile
                reasoningEffort
              }}
            }}
            "#
            ))
            .await;
        assert!(
            !save_without_reasoning.errors.is_empty(),
            "missing reasoning effort should be rejected"
        );
        assert_eq!(
            save_without_reasoning.errors[0].message,
            "reasoning effort is required for selected model profile"
        );

        let save_with_reasoning = schema
            .execute(format!(
                r#"
            mutation {{
              saveAgentModelPreference(input: {{
                agentId: "agent:primary",
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5",
                reasoningEffort: HIGH
              }}) {{
                modelProfile
                reasoningEffort
              }}
            }}
            "#
            ))
            .await;
        assert!(
            save_with_reasoning.errors.is_empty(),
            "{:?}",
            save_with_reasoning.errors
        );
        let data = save_with_reasoning.data.into_json().expect("json");
        assert_eq!(data["saveAgentModelPreference"]["reasoningEffort"], "HIGH");
    }

    #[tokio::test]
    async fn agents_query_does_not_invent_remote_model_profiles() {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        store
            .ensure_default_provider_account()
            .await
            .expect("codex account");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  agents {
                    modelOptions {
                      providerKind
                      profiles {
                        id
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let codex_option = data["agents"][0]["modelOptions"]
            .as_array()
            .expect("options")
            .iter()
            .find(|option| option["providerKind"] == "codex")
            .expect("codex option");
        assert!(
            codex_option["profiles"]
                .as_array()
                .expect("codex profiles")
                .is_empty()
        );
    }
