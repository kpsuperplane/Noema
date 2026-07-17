    #[tokio::test]
    async fn save_tool_calibration_mutation_persists_reviewed_policy() {
        use crate::test_support::test_store;
        use noema_capabilities_mcp::McpCalibrationStatus;

        let store = test_store().await;
        let seeded = seed_google_mcp_tools(&store, &[("read_doc", "fingerprint_1")]).await;
        let read_doc_id = seeded_tool_id(&seeded, "read_doc");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = execute_graphql_with_variables(
            &schema,
            r#"
                mutation SaveToolCalibration($mcpToolId: String!) {
                  saveToolCalibration(input: {
                    calibrationId: "tool_calibration:read_doc"
                    mcpToolId: $mcpToolId
                    readClassification: "mixed"
                    writeClassification: "none"
                    exportClassification: "none"
                    status: "blocked_unresolved_ownership"
                    reviewedBy: "human:local"
                    reviewedMetadataFingerprint: "fingerprint_1"
                  }) {
                    calibrationId
                    mcpToolId
                    status
                    readClassification
                    writeClassification
                    exportClassification
                  }
                }
                "#,
            json!({"mcpToolId": read_doc_id}),
        )
        .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let calibration = &data["saveToolCalibration"];
        assert_eq!(calibration["calibrationId"], "tool_calibration:read_doc");
        assert_eq!(calibration["mcpToolId"], read_doc_id);
        assert_eq!(calibration["status"], "blocked_unresolved_ownership");
        assert_eq!(calibration["readClassification"], "mixed");
        assert_eq!(calibration["writeClassification"], "none");
        assert_eq!(calibration["exportClassification"], "none");

        let persisted = store
            .invocation_snapshot(seeded.server.mcp_server_id, read_doc_id)
            .await
            .expect("invocation snapshot")
            .expect("tool exists")
            .calibration
            .expect("calibration exists");
        assert_eq!(
            persisted.status,
            McpCalibrationStatus::BlockedUnresolvedOwnership
        );
        assert_eq!(persisted.reviewed_by.as_deref(), Some("human:local"));
        assert_eq!(
            persisted.reviewed_metadata_fingerprint.as_deref(),
            Some("fingerprint_1")
        );
    }
    #[tokio::test]
    async fn save_tool_calibrations_mutation_persists_multiple_policies_in_one_request() {
        use crate::test_support::test_store;
        use noema_capabilities_mcp::{McpCalibrationStatus, McpTrustClassification};

        let store = test_store().await;
        let seeded = seed_google_mcp_tools(
            &store,
            &[
                ("read_doc", "fingerprint_read"),
                ("share_doc", "fingerprint_share"),
            ],
        )
        .await;
        let read_doc_id = seeded_tool_id(&seeded, "read_doc");
        let share_doc_id = seeded_tool_id(&seeded, "share_doc");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = execute_graphql_with_variables(
            &schema,
            r#"
                mutation SaveToolCalibrations($readDocId: String!, $shareDocId: String!) {
                  saveToolCalibrations(inputs: [
                    {
                      calibrationId: "tool_calibration:read_doc"
                      mcpToolId: $readDocId
                      readClassification: "mixed"
                      writeClassification: "none"
                      exportClassification: "none"
                      status: "blocked_unresolved_ownership"
                      reviewedBy: "human:local"
                      reviewedMetadataFingerprint: "fingerprint_read"
                    },
                    {
                      calibrationId: "tool_calibration:share_doc"
                      mcpToolId: $shareDocId
                      readClassification: "none"
                      writeClassification: "trusted"
                      exportClassification: "untrusted"
                      status: "ready"
                      reviewedBy: "human:local"
                      reviewedMetadataFingerprint: "fingerprint_share"
                    }
                  ]) {
                    mcpToolId
                    status
                    readClassification
                    writeClassification
                    exportClassification
                  }
                }
                "#,
            json!({"readDocId": read_doc_id, "shareDocId": share_doc_id}),
        )
        .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let calibrations = data["saveToolCalibrations"]
            .as_array()
            .expect("calibrations");
        assert_eq!(calibrations.len(), 2);
        assert_eq!(calibrations[0]["mcpToolId"], read_doc_id);
        assert_eq!(calibrations[1]["mcpToolId"], share_doc_id);

        let read_doc = store
            .invocation_snapshot(seeded.server.mcp_server_id.clone(), read_doc_id)
            .await
            .expect("read invocation snapshot")
            .expect("read tool exists")
            .calibration
            .expect("read calibration exists");
        let share_doc = store
            .invocation_snapshot(seeded.server.mcp_server_id, share_doc_id)
            .await
            .expect("share invocation snapshot")
            .expect("share tool exists")
            .calibration
            .expect("share calibration exists");
        assert_eq!(
            read_doc.status,
            McpCalibrationStatus::BlockedUnresolvedOwnership
        );
        assert_eq!(
            share_doc.write_classification,
            McpTrustClassification::Trusted
        );
        assert_eq!(
            share_doc.export_classification,
            McpTrustClassification::Untrusted
        );
    }

    #[tokio::test]
    async fn save_tool_calibrations_mutation_rejects_invalid_batch_without_partial_writes() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let seeded = seed_google_mcp_tools(
            &store,
            &[
                ("read_doc", "fingerprint_read"),
                ("share_doc", "fingerprint_share"),
            ],
        )
        .await;
        let read_doc_id = seeded_tool_id(&seeded, "read_doc");
        let share_doc_id = seeded_tool_id(&seeded, "share_doc");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = execute_graphql_with_variables(
            &schema,
            r#"
                mutation SaveToolCalibrations($readDocId: String!, $shareDocId: String!) {
                  saveToolCalibrations(inputs: [
                    {
                      calibrationId: "tool_calibration:read_doc"
                      mcpToolId: $readDocId
                      readClassification: "mixed"
                      writeClassification: "none"
                      exportClassification: "none"
                      status: "blocked_unresolved_ownership"
                      reviewedBy: "human:local"
                      reviewedMetadataFingerprint: "fingerprint_read"
                    },
                    {
                      calibrationId: "tool_calibration:share_doc"
                      mcpToolId: $shareDocId
                      readClassification: "none"
                      writeClassification: "trusted"
                      exportClassification: "untrusted"
                      status: "ready"
                      reviewedBy: "human:local"
                      reviewedMetadataFingerprint: "wrong_fingerprint"
                    }
                  ]) {
                    mcpToolId
                  }
                }
                "#,
            json!({"readDocId": read_doc_id, "shareDocId": share_doc_id}),
        )
        .await;

        assert!(!response.errors.is_empty());
        assert!(
            store
                .invocation_snapshot(seeded.server.mcp_server_id.clone(), read_doc_id,)
                .await
                .expect("read invocation snapshot")
                .expect("read tool exists")
                .calibration
                .is_none()
        );
        assert!(
            store
                .invocation_snapshot(seeded.server.mcp_server_id, share_doc_id)
                .await
                .expect("share invocation snapshot")
                .expect("share tool exists")
                .calibration
                .is_none()
        );
    }

    #[tokio::test]
    async fn save_tool_calibration_mutation_rejects_invalid_enum_strings() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let seeded = seed_google_mcp_tools(&store, &[("read_doc", "fingerprint_1")]).await;
        let read_doc_id = seeded_tool_id(&seeded, "read_doc");

        let schema = build_schema(GraphqlState::for_tests_with_store(store.clone()));
        let response = execute_graphql_with_variables(
            &schema,
            r#"
                mutation SaveToolCalibration($mcpToolId: String!) {
                  saveToolCalibration(input: {
                    calibrationId: "tool_calibration:read_doc"
                    mcpToolId: $mcpToolId
                    readClassification: "Mixed"
                    writeClassification: "none"
                    exportClassification: "none"
                    status: "blocked_unresolved_ownership"
                  }) {
                    calibrationId
                  }
                }
                "#,
            json!({"mcpToolId": read_doc_id}),
        )
        .await;

        assert!(!response.errors.is_empty());
        assert!(
            response.errors[0]
                .message
                .contains("invalid readClassification"),
            "{:?}",
            response.errors
        );
        assert!(
            store
                .invocation_snapshot(seeded.server.mcp_server_id, read_doc_id)
                .await
                .expect("invocation snapshot")
                .expect("tool exists")
                .calibration
                .is_none()
        );
    }

    #[tokio::test]
    async fn autofill_tool_calibrations_returns_validated_suggestions_without_persisting() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let seeded = seed_autofill_server(&store).await;
        let server_id = seeded.server.mcp_server_id.clone();
        let read_doc_id = seeded_tool_id(&seeded, "read_doc");
        let runtime = test_autofill_runtime(
            store.clone(),
            r#"{"suggestions":[{"tool":"read_doc","read":"m","write":"n","export":"n","d":false}]}"#,
        )
        .await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        let response = execute_graphql_with_variables(
            &schema,
            r#"
                mutation AutofillToolCalibrations($mcpServerId: String!) {
                  autofillToolCalibrations(mcpServerId: $mcpServerId) {
                    suggestions {
                      mcpToolId
                      readClassification
                      writeClassification
                      exportClassification
                      disabled
                    }
                  }
                }
                "#,
            json!({"mcpServerId": server_id}),
        )
        .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let suggestion = &data["autofillToolCalibrations"]["suggestions"][0];
        assert_eq!(suggestion["mcpToolId"], read_doc_id);
        assert_eq!(suggestion["readClassification"], "mixed");
        assert_eq!(suggestion["writeClassification"], "none");
        assert_eq!(suggestion["exportClassification"], "none");
        assert_eq!(suggestion["disabled"], false);
        assert!(
            store
                .invocation_snapshot(server_id, read_doc_id)
                .await
                .expect("invocation snapshot")
                .expect("tool exists")
                .calibration
                .is_none()
        );
    }

    #[tokio::test]
    async fn autofill_tool_calibrations_rejects_invalid_model_output_without_persisting() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let seeded = seed_autofill_server(&store).await;
        let server_id = seeded.server.mcp_server_id.clone();
        let read_doc_id = seeded_tool_id(&seeded, "read_doc");
        let runtime = test_autofill_runtime(
            store.clone(),
            r#"{"suggestions":[{"tool":"missing_doc","read":"m","write":"n","export":"n","d":false}]}"#,
        )
        .await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        let response = execute_graphql_with_variables(
            &schema,
            r#"
                mutation AutofillToolCalibrations($mcpServerId: String!) {
                  autofillToolCalibrations(mcpServerId: $mcpServerId) {
                    suggestions { mcpToolId }
                  }
                }
                "#,
            json!({"mcpServerId": server_id}),
        )
        .await;

        assert!(!response.errors.is_empty());
        assert_eq!(
            response.errors[0].message,
            "the MCP server returned an unsupported response"
        );
        assert!(!response.errors[0].message.contains("missing_doc"));
        assert!(
            store
                .invocation_snapshot(server_id, read_doc_id)
                .await
                .expect("invocation snapshot")
                .expect("tool exists")
                .calibration
                .is_none()
        );
    }

    #[tokio::test]
    async fn autofill_tool_calibrations_returns_null_when_disabled_is_omitted() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let seeded = seed_autofill_server(&store).await;
        let server_id = seeded.server.mcp_server_id.clone();
        let read_doc_id = seeded_tool_id(&seeded, "read_doc");
        let runtime = test_autofill_runtime(
            store.clone(),
            r#"{"suggestions":[{"tool":"read_doc","read":"m","write":"n","export":"n"}]}"#,
        )
        .await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        let response = execute_graphql_with_variables(
            &schema,
            r#"
                mutation AutofillToolCalibrations($mcpServerId: String!) {
                  autofillToolCalibrations(mcpServerId: $mcpServerId) {
                    suggestions {
                      mcpToolId
                      disabled
                    }
                  }
                }
                "#,
            json!({"mcpServerId": server_id}),
        )
        .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let suggestion = &data["autofillToolCalibrations"]["suggestions"][0];
        assert_eq!(suggestion["mcpToolId"], read_doc_id);
        assert_eq!(suggestion["disabled"], serde_json::Value::Null);
        assert!(
            store
                .invocation_snapshot(server_id, read_doc_id)
                .await
                .expect("invocation snapshot")
                .expect("tool exists")
                .calibration
                .is_none()
        );
    }

    async fn seed_autofill_server(
        store: &noema_store::NoemaStore,
    ) -> noema_capabilities_mcp::McpControlPlaneServer {
        use noema_capabilities_mcp::{
            McpDiscoveredTool, McpInitialDiscoveryCommit, McpServerAuthStatus, McpTransportKind,
            NewMcpServer,
        };

        store
            .commit_initial_discovery(McpInitialDiscoveryCommit {
                server: NewMcpServer {
                    display_name: "Docs".to_string(),
                    transport_kind: McpTransportKind::Stdio,
                    safe_config: json!({}),
                },
                tools: vec![McpDiscoveredTool {
                    name: "read_doc".to_string(),
                    description: Some("Read a document by id".to_string()),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "owner_email": { "type": "string" }
                        }
                    }),
                    output_schema: None,
                    annotations: json!({"readOnlyHint": true}),
                    metadata_fingerprint: "fingerprint_1".to_string(),
                }],
                auth_status: McpServerAuthStatus::None,
            })
            .await
            .expect("commit Docs discovery")
    }

    #[tokio::test]
    async fn autofill_tool_calibrations_uses_runtime_tool_classification_model() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let seeded = seed_autofill_server(&store).await;
        let (requests, runtime) = test_autofill_runtime_with_requests(
            store.clone(),
            r#"{"suggestions":[{"tool":"read_doc","read":"m","write":"n","export":"n","d":false}]}"#,
            Some("gpt-test-tool-classifier"),
        )
        .await;
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store.clone(),
            runtime,
        ));

        let response = execute_graphql_with_variables(
            &schema,
            r#"
                mutation AutofillToolCalibrations($mcpServerId: String!) {
                  autofillToolCalibrations(mcpServerId: $mcpServerId) {
                    suggestions { mcpToolId }
                  }
                }
                "#,
            json!({"mcpServerId": seeded.server.mcp_server_id}),
        )
        .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let requests = requests.lock().expect("requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].model.as_deref(),
            Some("gpt-test-tool-classifier")
        );
    }

    #[tokio::test]
    async fn autofill_tool_calibrations_reads_classification_model_from_replacement_route() {
        use crate::test_support::test_store;

        let store = test_store().await;
        let seeded = seed_autofill_server(&store).await;
        let old_requests = Arc::new(Mutex::new(Vec::new()));
        let old_provider = noema_providers::erase_model_provider(AutofillTestProvider {
            text: r#"{"suggestions":[]}"#.to_string(),
            tool_classification_model: Some("old-classifier".to_string()),
            requests: old_requests.clone(),
        });
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
            .expect("authenticate provider account");
        let instance_key =
            noema_providers::provider_account_instance_key("provider_account:codex:default")
                .expect("provider instance key");
        let mut configured_default = noema_providers::ProviderSelectionSnapshot::explicit(
            "codex",
            "provider_account:codex:default",
            "gpt-test",
            None,
            Some("test_configured_default".to_string()),
        );
        configured_default.provider_instance_key = Some(instance_key.clone());
        let registry = Arc::new(noema_providers::ProviderRegistry::new());
        registry
            .register(instance_key.clone(), old_provider)
            .expect("register old provider");
        let ready_selection = registry
            .prove_ready_selection(configured_default.clone())
            .expect("ready configured selection");
        store
            .initialize_missing_provider_selections(&configured_default, Some(&ready_selection))
            .await
            .expect("initialize provider selections");
        let runtime = crate::test_support::spawn_runtime_with_provider_registry_and_memory(
            registry.clone(),
            store.clone(),
            crate::test_support::artifact_operations(&store).expect("artifact operations"),
            None,
            noema_runtime::RuntimeEventRegistry::default(),
        )
        .await
        .expect("runtime");
        let replacement_requests = Arc::new(Mutex::new(Vec::new()));
        let replacement =
            noema_providers::erase_model_provider(AutofillTestProvider {
                text: r#"{"suggestions":[{"tool":"read_doc","read":"m","write":"n","export":"n","d":false}]}"#.to_string(),
                tool_classification_model: Some("replacement-classifier".to_string()),
                requests: replacement_requests.clone(),
            });
        registry
            .register(instance_key, replacement)
            .expect("publish replacement");
        let schema = build_schema(GraphqlState::for_tests_with_store_and_runtime(
            store, runtime,
        ));

        let response = execute_graphql_with_variables(
            &schema,
            r#"
                mutation AutofillToolCalibrations($mcpServerId: String!) {
                  autofillToolCalibrations(mcpServerId: $mcpServerId) {
                    suggestions { mcpToolId }
                  }
                }
                "#,
            json!({"mcpServerId": seeded.server.mcp_server_id}),
        )
        .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert!(old_requests.lock().expect("old requests").is_empty());
        let replacement_requests = replacement_requests.lock().expect("replacement requests");
        assert_eq!(replacement_requests.len(), 1);
        assert_eq!(
            replacement_requests[0].model.as_deref(),
            Some("replacement-classifier")
        );
    }
