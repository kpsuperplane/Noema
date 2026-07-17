    #[tokio::test]
    async fn web_fetch_settings_query_defaults_to_tool_model_and_returns_options() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_metadata(
                &codex.provider_account_id,
                serde_json::json!({
                    "profiles": [
                        { "id": "gpt-5.4-mini", "label": "GPT-5.4 Mini" },
                        { "id": "gpt-5.5", "label": "GPT-5.5" }
                    ]
                }),
            )
            .await
            .expect("codex metadata");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  webFetchSettings {
                    summarizer {
                      defaultModelProfile
                      modelPreference {
                        providerKind
                      }
                      modelOptions {
                        providerKind
                        providerAccountId
                        providerDisplayName
                        status
                        disabledReason
                        profiles {
                          id
                          label
                          disabledReason
                        }
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let summarizer = &data["webFetchSettings"]["summarizer"];
        assert_eq!(summarizer["defaultModelProfile"], "gpt-5.4-mini");
        assert_eq!(summarizer["modelPreference"], serde_json::Value::Null);
        assert_eq!(summarizer["modelOptions"][0]["providerKind"], "codex");
        assert_eq!(
            summarizer["modelOptions"][0]["profiles"][0]["id"],
            "gpt-5.4-mini"
        );
    }

    #[tokio::test]
    async fn web_tool_settings_query_returns_default_system_bindings() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  webToolSettings {
                    search {
                      toolName
                      capabilityId
                      activeProviderAccountId
                      providerOptions {
                        providerAccountId
                        providerKind
                      }
                    }
                    fetch {
                      toolName
                      capabilityId
                      activeProviderAccountId
                      providerOptions {
                        providerAccountId
                        providerKind
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(
            data["webToolSettings"]["search"]["activeProviderAccountId"],
            "provider_account:duckduckgo_public:system"
        );
        assert_eq!(
            data["webToolSettings"]["fetch"]["activeProviderAccountId"],
            "provider_account:direct_http:system"
        );
        assert_eq!(
            data["webToolSettings"]["search"]["providerOptions"][0]["providerKind"],
            "duckduckgo_public"
        );
        assert_eq!(
            data["webToolSettings"]["fetch"]["providerOptions"][0]["providerKind"],
            "direct_http"
        );
    }

    #[tokio::test]
    async fn save_web_tool_provider_binding_mutation_returns_saved_binding() {
        use crate::test_support::test_store;
        use noema_providers::ProviderAccountStatus;

        let store = test_store().await;
        let provider_account_id = crate::test_support::create_exa_provider_account_for_tests(
            &store,
            "Exa research",
            ProviderAccountStatus::Authenticated,
            json!({}),
        )
        .await
        .provider_account_id;

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveWebToolProviderBinding(input: {{
                    toolName: "web.search"
                    capabilityId: "web.search"
                    providerAccountId: "{provider_account_id}"
                  }}) {{
                    toolName
                    capabilityId
                    activeProviderAccountId
                  }}
                }}
                "#,
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(data["saveWebToolProviderBinding"]["toolName"], "web.search");
        assert_eq!(
            data["saveWebToolProviderBinding"]["activeProviderAccountId"],
            provider_account_id
        );
    }

    #[tokio::test]
    async fn save_web_fetch_summarizer_preference_persists_valid_codex_profile() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_status(
                &codex.provider_account_id,
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("codex authenticated");
        store
            .update_provider_account_metadata(
                &codex.provider_account_id,
                serde_json::json!({
                    "profiles": [
                        { "id": "gpt-5.4-mini", "label": "GPT-5.4 Mini" }
                    ]
                }),
            )
            .await
            .expect("codex metadata");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveWebFetchSummarizerPreference(input: {{
                    providerAccountId: "{}"
                    modelProfile: "gpt-5.4-mini"
                  }}) {{
                    providerKind
                    providerAccountId
                    modelProfile
                  }}
                }}
                "#,
                codex.provider_account_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let saved = store
            .get_auxiliary_model_preference(noema_store::WEB_FETCH_SUMMARIZER_TASK_ID)
            .await
            .expect("preference read")
            .expect("preference saved");
        assert_eq!(saved.provider_kind, "codex");
        assert_eq!(saved.provider_account_id, codex.provider_account_id);
        assert_eq!(saved.model_profile, "gpt-5.4-mini");
    }

    #[tokio::test]
    async fn save_web_fetch_summarizer_preference_persists_reasoning_effort() {
        let (schema, account_id) = schema_with_reasoning_profile().await;
        let response = schema
            .execute(format!(
                r#"
            mutation {{
              saveWebFetchSummarizerPreference(input: {{
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5",
                reasoningEffort: LOW
              }}) {{
                modelProfile
                reasoningEffort
              }}
            }}
            "#
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(
            data["saveWebFetchSummarizerPreference"]["reasoningEffort"],
            "LOW"
        );
    }

    #[tokio::test]
    async fn usage_settings_query_exposes_progress_audit_default() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_metadata(
                &codex.provider_account_id,
                serde_json::json!({
                    "profiles": [
                        { "id": "gpt-5.4-mini", "label": "GPT-5.4 Mini" },
                        { "id": "gpt-5.5", "label": "GPT-5.5" }
                    ]
                }),
            )
            .await
            .expect("codex metadata");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  usageSettings {
                    progressAudit {
                      defaultModelProfile
                      modelPreference { providerKind }
                      modelOptions { providerKind profiles { id } }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let audit = &data["usageSettings"]["progressAudit"];
        assert_eq!(audit["defaultModelProfile"], "gpt-5.4-mini");
        assert_eq!(audit["modelPreference"], serde_json::Value::Null);
        assert_eq!(
            audit["modelOptions"][0]["profiles"][0]["id"],
            "gpt-5.4-mini"
        );
    }

    #[tokio::test]
    async fn usage_settings_query_exposes_provider_specific_progress_audit_defaults() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_metadata(
                &codex.provider_account_id,
                serde_json::json!({
                    "profiles": [
                        { "id": "gpt-5.4-mini", "label": "GPT-5.4 Mini" },
                        { "id": "gpt-5.5", "label": "GPT-5.5" }
                    ]
                }),
            )
            .await
            .expect("codex metadata");
        store
            .ensure_default_foundation_local_provider_account()
            .await
            .expect("foundation account");

        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                {
                  usageSettings {
                    progressAudit {
                      modelOptions {
                        providerKind
                        defaultModelProfile
                        profiles { id }
                      }
                    }
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let options = data["usageSettings"]["progressAudit"]["modelOptions"]
            .as_array()
            .expect("model options");
        let codex = options
            .iter()
            .find(|option| option["providerKind"] == "codex")
            .expect("codex option");
        let foundation = options
            .iter()
            .find(|option| option["providerKind"] == "foundation_local")
            .expect("foundation option");

        assert_eq!(codex["defaultModelProfile"], "gpt-5.4-mini");
        assert_eq!(foundation["defaultModelProfile"], "default");
        assert_ne!(foundation["defaultModelProfile"], "gpt-5.4-mini");
    }

    #[tokio::test]
    async fn save_tool_progress_audit_preference_persists_valid_codex_profile() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_status(
                &codex.provider_account_id,
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("codex authenticated");
        store
            .update_provider_account_metadata(
                &codex.provider_account_id,
                serde_json::json!({
                    "profiles": [
                        { "id": "gpt-5.4-mini", "label": "GPT-5.4 Mini" }
                    ]
                }),
            )
            .await
            .expect("codex metadata");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveToolProgressAuditPreference(input: {{
                    providerAccountId: "{}"
                    modelProfile: "gpt-5.4-mini"
                  }}) {{
                    providerKind
                    providerAccountId
                    modelProfile
                  }}
                }}
                "#,
                codex.provider_account_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let saved = store
            .get_auxiliary_model_preference(noema_store::TOOL_PROGRESS_AUDIT_TASK_ID)
            .await
            .expect("preference read")
            .expect("preference saved");
        assert_eq!(saved.provider_kind, "codex");
        assert_eq!(saved.provider_account_id, codex.provider_account_id);
        assert_eq!(saved.model_profile, "gpt-5.4-mini");
    }

    #[tokio::test]
    async fn save_tool_progress_audit_preference_persists_reasoning_effort() {
        let (schema, account_id) = schema_with_reasoning_profile().await;
        let response = schema
            .execute(format!(
                r#"
            mutation {{
              saveToolProgressAuditPreference(input: {{
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5",
                reasoningEffort: MEDIUM
              }}) {{
                modelProfile
                reasoningEffort
              }}
            }}
            "#
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(
            data["saveToolProgressAuditPreference"]["reasoningEffort"],
            "MEDIUM"
        );
    }

    #[tokio::test]
    async fn save_web_fetch_summarizer_preference_rejects_unavailable_provider() {
        use crate::test_support::test_store;

        let store = test_store().await;
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
                  saveWebFetchSummarizerPreference(input: {{
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
                .get_auxiliary_model_preference(noema_store::WEB_FETCH_SUMMARIZER_TASK_ID)
                .await
                .expect("preference read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn save_web_fetch_summarizer_preference_rejects_unavailable_profile() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let codex = store
            .ensure_default_provider_account()
            .await
            .expect("codex account");
        store
            .update_provider_account_status(
                &codex.provider_account_id,
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("codex authenticated");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveWebFetchSummarizerPreference(input: {{
                    providerAccountId: "{}"
                    modelProfile: "gpt-5.4-mini"
                  }}) {{
                    providerKind
                  }}
                }}
                "#,
                codex.provider_account_id
            )))
            .await;

        assert_eq!(response.errors.len(), 1);
        assert_eq!(
            response.errors[0].message,
            "model profile is not available for provider"
        );
        assert!(
            store
                .get_auxiliary_model_preference(noema_store::WEB_FETCH_SUMMARIZER_TASK_ID)
                .await
                .expect("preference read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn save_agent_model_preference_mutation_persists_valid_profile() {
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
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("foundation available");

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
                    providerAccountId
                    modelProfile
                  }}
                }}
                "#,
                foundation.provider_account_id
            )))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let saved = store
            .get_agent_runtime_preference("agent:primary")
            .await
            .expect("preference read")
            .expect("preference saved");
        assert_eq!(saved.provider_kind, "foundation_local");
        assert_eq!(saved.model_profile, "default");
    }

    #[tokio::test]
    async fn save_agent_model_preference_rejects_task_executor_identity() {
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
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("foundation available");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = schema
            .execute(async_graphql::Request::new(format!(
                r#"
                mutation {{
                  saveAgentModelPreference(input: {{
                    agentId: "agent:task-executor"
                    providerAccountId: "{}"
                    modelProfile: "default"
                  }}) {{
                    modelProfile
                  }}
                }}
                "#,
                foundation.provider_account_id
            )))
            .await;

        assert_eq!(response.errors.len(), 1);
        assert!(response.errors[0].message.contains("complexity tier"));
        assert!(
            store
                .get_agent_runtime_preference(noema_tasks::TASK_EXECUTOR_AGENT_ID)
                .await
                .expect("preference read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn save_agent_model_preference_requires_reasoning_for_reasoning_profile() {
        let (schema, account_id) = schema_with_reasoning_profile().await;
        let response = schema
            .execute(format!(
                r#"
            mutation {{
              saveAgentModelPreference(input: {{
                agentId: "agent:primary",
                providerAccountId: "{account_id}",
                modelProfile: "gpt-5.5"
              }}) {{
                modelProfile
              }}
            }}
            "#
            ))
            .await;
        assert!(!response.errors.is_empty());
        assert!(response.errors[0].message.contains("reasoning"));
    }

    #[tokio::test]
    async fn save_agent_model_preference_persists_reasoning_effort() {
        let (schema, account_id) = schema_with_reasoning_profile().await;
        let response = schema
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
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        assert_eq!(data["saveAgentModelPreference"]["reasoningEffort"], "HIGH");
    }
