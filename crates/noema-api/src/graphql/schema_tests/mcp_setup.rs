    #[tokio::test]
    async fn mcp_settings_query_returns_servers() {
        use crate::test_support::test_store;
        use noema_capabilities_mcp::{
            McpDiscoveredTool, McpFailureStatus, McpInitialDiscoveryCommit, McpServerAuthStatus,
            McpServerHealthStatus, McpTransportKind, NewMcpServer,
        };

        let store = test_store().await;
        let local = store
            .commit_initial_discovery(McpInitialDiscoveryCommit {
                server: NewMcpServer {
                    display_name: "Local Test".to_string(),
                    transport_kind: McpTransportKind::Stdio,
                    safe_config: json!({"command": "test-mcp"}),
                },
                tools: vec![McpDiscoveredTool {
                    name: "read".to_string(),
                    description: Some("Read metadata".to_string()),
                    input_schema: json!({"type": "object"}),
                    output_schema: Some(json!({"type": "object"})),
                    annotations: json!({"readOnlyHint": true}),
                    metadata_fingerprint: "fingerprint:local-test:read:v1".to_string(),
                }],
                auth_status: McpServerAuthStatus::None,
            })
            .await
            .expect("commit local discovery");
        assert!(
            store
                .record_failure_status(McpFailureStatus {
                    mcp_server_id: local.server.mcp_server_id.clone(),
                    expected_authority_generation: local.server.authority_generation.clone(),
                    health_status: McpServerHealthStatus::Unknown,
                    auth_status: McpServerAuthStatus::None,
                })
                .await
                .expect("restore unknown health projection")
        );
        let browser_oauth = store
            .commit_initial_discovery(McpInitialDiscoveryCommit {
                server: NewMcpServer {
                    display_name: "Browser OAuth".to_string(),
                    transport_kind: McpTransportKind::StreamableHttp,
                    safe_config: json!({
                        "url": "https://example.com/mcp",
                        "headers": {},
                        "secret_refs": { "oauth_credentials": true }
                    }),
                },
                tools: Vec::new(),
                auth_status: McpServerAuthStatus::Authenticated,
            })
            .await
            .expect("commit browser OAuth discovery");
        let local_server_id = local.server.mcp_server_id;
        let browser_oauth_server_id = browser_oauth.server.mcp_server_id;
        let schema = build_schema(GraphqlState::for_tests_with_store(store));
        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                query McpSettings {
                  mcpServers {
                    mcpServerId
                    displayName
                    transportKind
                    enabled
                    healthStatus
                    toolCount
                    browserOauthReauthenticationSupported
                  }
                }
                "#,
            ))
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let server = data["mcpServers"]
            .as_array()
            .expect("servers")
            .iter()
            .find(|server| server["mcpServerId"] == local_server_id)
            .expect("local test server");
        assert_eq!(server["mcpServerId"], local_server_id);
        assert_eq!(server["displayName"], "Local Test");
        assert_eq!(server["transportKind"], "stdio");
        assert_eq!(server["enabled"], false);
        assert_eq!(server["healthStatus"], "unknown");
        assert_eq!(server["toolCount"], 1);
        assert_eq!(server["browserOauthReauthenticationSupported"], false);

        let browser_oauth_server = data["mcpServers"]
            .as_array()
            .expect("servers")
            .iter()
            .find(|server| server["mcpServerId"] == browser_oauth_server_id)
            .expect("browser oauth server");
        assert_eq!(
            browser_oauth_server["browserOauthReauthenticationSupported"],
            true
        );
    }

    #[tokio::test]
    async fn create_mcp_server_mutation_returns_ready_for_calibration_without_secrets() {
        let fixture =
            GraphqlMcpSetupFixture::new(vec![TestMcpSetupOutcome::Ok(vec![discovered_mcp_tool(
                "list_repos",
                "List repositories",
                Some(json!({"type": "object"})),
                json!({"readOnlyHint": true}),
            )])])
            .await;
        let schema = build_schema(fixture.state);
        let response = execute_graphql(
            &schema,
            r#"
                mutation {
                  createMcpServer(input: {
                    displayName: "GitHub"
                    transportKind: "stdio"
                    stdio: {
                      command: "npx"
                      args: ["-y", "server"]
                      env: { GITHUB_OWNER: "example" }
                      secretEnv: { GITHUB_TOKEN: "top-secret" }
                    }
                  }) {
                    setupStatus
                    discoveryStatus
                    discoveredToolCount
                    setupError
                    server {
                      mcpServerId
                      displayName
                      transportKind
                      enabled
                      healthStatus
                      authStatus
                      toolCount
                    }
                  }
                }
                "#,
        )
        .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let result = &data["createMcpServer"];
        assert_eq!(result["setupStatus"], "ready_for_calibration");
        assert_eq!(result["discoveryStatus"], "discovered");
        assert_eq!(result["discoveredToolCount"], 1);
        assert!(
            result["server"]["mcpServerId"]
                .as_str()
                .is_some_and(|id| id.starts_with("mcp_server:"))
        );
        assert_eq!(result["server"]["toolCount"], 1);
        assert!(
            !serde_json::to_string(&data)
                .expect("response json")
                .contains("top-secret")
        );
    }

    #[tokio::test]
    async fn create_mcp_server_mutation_reports_browser_oauth_before_persisting() {
        let fixture = GraphqlMcpSetupFixture::new(vec![TestMcpSetupOutcome::AuthRequired(
            "authorization required".to_string(),
        )])
        .await;
        let store = fixture.store.clone();
        let schema = build_schema(fixture.state);
        let response = execute_graphql(
            &schema,
            r#"
                mutation {
                  createMcpServer(input: {
                    displayName: "Dex"
                    transportKind: "streamable_http"
                    http: { url: "https://mcp.getdex.com/mcp" }
                  }) {
                    setupStatus
                    discoveryStatus
                    discoveredToolCount
                    setupError
                    auth {
                      oauthAuthorizationSupported
                      oauthClientCredentialsSupported
                    }
                    server { mcpServerId }
                  }
                }
                "#,
        )
        .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().expect("json");
        let result = &data["createMcpServer"];
        assert_eq!(result["setupStatus"], "needs_auth");
        assert_eq!(result["discoveryStatus"], "needs_auth");
        assert_eq!(result["discoveredToolCount"], 0);
        assert_eq!(result["auth"]["oauthAuthorizationSupported"], true);
        assert_eq!(result["auth"]["oauthClientCredentialsSupported"], true);
        assert_eq!(result["server"], serde_json::Value::Null);
        assert!(
            store
                .control_plane_catalog()
                .await
                .expect("list servers")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn create_mcp_server_mutation_rejects_secret_shaped_safe_keys() {
        let fixture = GraphqlMcpSetupFixture::new(Vec::new()).await;
        let schema = build_schema(fixture.state);
        let response = execute_graphql(
            &schema,
            r#"
                mutation {
                  createMcpServer(input: {
                    displayName: "Unsafe"
                    transportKind: "streamable_http"
                    http: {
                      url: "https://example.com/mcp"
                      headers: { Authorization: "Bearer unsafe" }
                    }
                  }) {
                    setupStatus
                  }
                }
                "#,
        )
        .await;

        assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
        assert_eq!(response.errors[0].message, "invalid MCP operation input");
        assert!(!response.errors[0].message.contains("Bearer unsafe"));
    }

    #[tokio::test]
    async fn create_mcp_server_mutation_does_not_persist_until_auth_and_discovery_succeed() {
        let fixture = GraphqlMcpSetupFixture::new(vec![
            TestMcpSetupOutcome::AuthRequired("missing authorization".to_string()),
            TestMcpSetupOutcome::Ok(vec![discovered_mcp_tool(
                "retry_tool",
                "Retry tool",
                None,
                json!({}),
            )]),
        ])
        .await;
        let schema = build_schema(fixture.state);
        let store = fixture.store.clone();
        let create = execute_graphql(
            &schema,
            r#"
                mutation {
                  createMcpServer(input: {
                    displayName: "Remote"
                    transportKind: "streamable_http"
                    http: { url: "https://example.com/mcp" }
                  }) {
                    setupStatus
                    setupError
                    server { mcpServerId authStatus }
                  }
                }
                "#,
        )
        .await;
        assert!(create.errors.is_empty(), "{:?}", create.errors);
        let create_data = create.data.into_json().expect("create json");
        assert_eq!(create_data["createMcpServer"]["setupStatus"], "needs_auth");
        assert_eq!(
            create_data["createMcpServer"]["server"],
            serde_json::Value::Null
        );
        assert!(
            store
                .control_plane_catalog()
                .await
                .expect("list servers")
                .is_empty()
        );

        let retry = execute_graphql(
            &schema,
            r#"
                mutation {
                  createMcpServer(input: {
                    displayName: "Remote"
                    transportKind: "streamable_http"
                    http: {
                      url: "https://example.com/mcp"
                      secretHeaders: { Authorization: "Bearer retry" }
                    }
                  }) {
                    setupStatus
                    discoveredToolCount
                    server { mcpServerId authStatus toolCount }
                  }
                }
                "#,
        )
        .await;

        assert!(retry.errors.is_empty(), "{:?}", retry.errors);
        let retry_data = retry.data.into_json().expect("retry json");
        assert_eq!(
            retry_data["createMcpServer"]["setupStatus"],
            "ready_for_calibration"
        );
        assert_eq!(retry_data["createMcpServer"]["discoveredToolCount"], 1);
        assert_eq!(retry_data["createMcpServer"]["server"]["toolCount"], 1);
        assert!(
            !serde_json::to_string(&retry_data)
                .expect("retry json")
                .contains("Bearer retry")
        );
    }

    #[tokio::test]
    async fn mcp_tools_query_and_delete_mutation_use_persisted_setup_state() {
        let fixture =
            GraphqlMcpSetupFixture::new(vec![TestMcpSetupOutcome::Ok(vec![discovered_mcp_tool(
                "read_doc",
                "Read a document",
                None,
                json!({"readOnlyHint": true}),
            )])])
            .await;
        let schema = build_schema(fixture.state);

        let create = execute_graphql(
            &schema,
            r#"
                mutation {
                  createMcpServer(input: {
                    displayName: "Docs"
                    transportKind: "stdio"
                    stdio: {
                      command: "docs-mcp"
                      args: []
                      secretEnv: { DOCS_TOKEN: "secret" }
                    }
                  }) {
                    setupStatus
                    server { mcpServerId toolCount }
                  }
                }
                "#,
        )
        .await;
        assert!(create.errors.is_empty(), "{:?}", create.errors);
        let create_data = create.data.into_json().expect("create json");
        let mcp_server_id = create_data["createMcpServer"]["server"]["mcpServerId"]
            .as_str()
            .expect("server id")
            .to_string();
        assert!(
            fixture
                .paths
                .mcp_server_home(&mcp_server_id)
                .join("secrets.json")
                .exists()
        );

        let tools_query = format!(
            r#"
                query {{
                  mcpTools(mcpServerId: "{mcp_server_id}") {{
                    mcpToolId
                    mcpServerId
                    name
                    description
                    inputSchema
                    annotations
                    metadataFingerprint
                    calibration {{ status }}
                  }}
                }}
                "#
        );
        let tools = execute_graphql(&schema, &tools_query).await;
        assert!(tools.errors.is_empty(), "{:?}", tools.errors);
        let tools_data = tools.data.into_json().expect("tools json");
        let tool = &tools_data["mcpTools"][0];
        let mcp_tool_id = tool["mcpToolId"].as_str().expect("tool id").to_string();
        assert_eq!(tool["mcpServerId"], mcp_server_id);
        assert_eq!(tool["name"], "read_doc");
        assert_eq!(tool["description"], "Read a document");
        assert_eq!(tool["inputSchema"], json!({"type": "object"}));
        assert_eq!(tool["calibration"], serde_json::Value::Null);

        let delete_mutation = format!(
            r#"
                mutation {{
                  deleteMcpServer(mcpServerId: "{mcp_server_id}")
                }}
                "#
        );
        let delete = execute_graphql(&schema, &delete_mutation).await;
        assert!(delete.errors.is_empty(), "{:?}", delete.errors);
        let delete_data = delete.data.into_json().expect("delete json");
        assert_eq!(delete_data["deleteMcpServer"], true);
        assert!(
            fixture
                .store
                .control_plane_server(mcp_server_id.clone())
                .await
                .expect("server")
                .is_none()
        );
        assert!(
            fixture
                .store
                .invocation_snapshot(mcp_server_id.clone(), mcp_tool_id)
                .await
                .expect("invocation snapshot")
                .is_none()
        );
        assert!(!fixture.paths.mcp_server_home(&mcp_server_id).exists());
    }

    struct GraphqlMcpSetupFixture {
        state: GraphqlState,
        store: noema_store::NoemaStore,
        paths: TestEnvironment,
        _home: TempDir,
    }

    impl GraphqlMcpSetupFixture {
        async fn new(outcomes: Vec<TestMcpSetupOutcome>) -> Self {
            let home = TempDir::new().expect("temp noema home");
            let paths = TestEnvironment::from_root(home.path()).expect("paths");
            let store = crate::test_support::test_store_for_environment(&paths).await;
            let mcp_operations = crate::test_support::test_mcp_operations_with_setup(
                store.clone(),
                None,
                paths.clone(),
                outcomes,
            );
            let state = GraphqlState::for_tests_with_store_and_environment(store.clone(), paths.clone())
                .with_mcp_operations(mcp_operations);
            Self {
                state,
                store,
                paths,
                _home: home,
            }
        }
    }

    async fn execute_graphql(schema: &GraphqlSchema, query: &str) -> async_graphql::Response {
        schema.execute(async_graphql::Request::new(query)).await
    }

    async fn execute_graphql_with_variables(
        schema: &GraphqlSchema,
        query: &str,
        variables: serde_json::Value,
    ) -> async_graphql::Response {
        schema
            .execute(
                async_graphql::Request::new(query)
                    .variables(async_graphql::Variables::from_json(variables)),
            )
            .await
    }

    fn discovered_mcp_tool(
        name: &str,
        description: &str,
        output_schema: Option<serde_json::Value>,
        annotations: serde_json::Value,
    ) -> noema_capabilities_mcp::McpDiscoveredTool {
        noema_capabilities_mcp::McpDiscoveredTool {
            name: name.to_string(),
            description: Some(description.to_string()),
            input_schema: json!({"type": "object"}),
            output_schema,
            annotations,
            metadata_fingerprint: String::new(),
        }
    }

    async fn seed_google_mcp_tools(
        store: &noema_store::NoemaStore,
        tools: &[(&str, &str)],
    ) -> noema_capabilities_mcp::McpControlPlaneServer {
        use noema_capabilities_mcp::{
            McpInitialDiscoveryCommit, McpServerAuthStatus, McpTransportKind, NewMcpServer,
        };

        store
            .commit_initial_discovery(McpInitialDiscoveryCommit {
                server: NewMcpServer {
                    display_name: "Google".to_string(),
                    transport_kind: McpTransportKind::Stdio,
                    safe_config: json!({}),
                },
                tools: tools
                    .iter()
                    .map(
                        |(name, fingerprint)| noema_capabilities_mcp::McpDiscoveredTool {
                            name: (*name).to_string(),
                            description: Some(format!("Tool {name}")),
                            input_schema: json!({"type": "object"}),
                            output_schema: None,
                            annotations: json!({}),
                            metadata_fingerprint: (*fingerprint).to_string(),
                        },
                    )
                    .collect(),
                auth_status: McpServerAuthStatus::None,
            })
            .await
            .expect("commit Google discovery")
    }

    fn seeded_tool_id(
        server: &noema_capabilities_mcp::McpControlPlaneServer,
        name: &str,
    ) -> String {
        server
            .tools
            .iter()
            .find(|tool| tool.tool.name == name)
            .map(|tool| tool.tool.mcp_tool_id.clone())
            .expect("seeded MCP tool")
    }
