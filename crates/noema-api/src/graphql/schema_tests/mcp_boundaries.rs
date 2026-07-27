use std::sync::{Arc, Mutex};

use noema_capabilities_mcp::{
    AddMcpConnectionCommand, CompleteMcpOAuthSetupCommand, ContinueMcpServerSetupCommand, CreateMcpServerCommand,
    McpDeleteServerCommand, McpDeleteServerResult, McpDiscoveryStatus, McpListToolsCommand,
    McpOAuthSetupAttemptQuery, McpOAuthSetupAttemptStatus, McpOAuthSetupAttemptView,
    McpOperationError, McpOperationFuture, McpOperationResult, McpOperations,
    McpResetToolPolicyCommand, McpSaveProviderPolicyCommand, McpSaveToolOverrideCommand,
    McpServerList, McpServerRecord, McpServerSetupResult, McpSetToolEnabledCommand,
    McpSetupAuthDetails, McpSetupIssue, McpSetupStatus, McpToolList, McpToolPolicyRecord,
    StartMcpOAuthReauthenticationCommand, StartMcpOAuthSetupCommand,
};

#[tokio::test]
async fn mcp_graphql_route_and_setup_boundaries() {
    // Case: create_mcp_server_mutation_reports_browser_oauth_before_persisting.
    use crate::test_support::test_store;
    use noema_capabilities_mcp::McpRepository;

    let store = test_store().await;
    let operations = Arc::new(McpBoundaryOperations::without_runtime());
    let schema = build_schema(
        GraphqlState::for_tests_with_store(store.clone()).with_mcp_operations(operations.clone()),
    );
    let response = schema
        .execute(
            r#"mutation {
              createMcpServer(input: {
                displayName: "Dex", transportKind: "streamable_http",
                http: { url: "https://mcp.getdex.com/mcp" }
              }) {
                setupStatus discoveryStatus discoveredToolCount setupError
                auth { oauthAuthorizationSupported oauthClientCredentialsSupported }
                server { mcpServerId }
              }
            }"#,
        )
        .await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    let result = &data["createMcpServer"];
    assert_json_fields!(result,
        "/setupStatus" => "needs_auth",
        "/discoveryStatus" => "needs_auth",
        "/discoveredToolCount" => 0,
        "/auth/oauthAuthorizationSupported" => true,
        "/auth/oauthClientCredentialsSupported" => true,
        "/server" => serde_json::Value::Null,
    );
    {
        let commands = operations.create_commands.lock().expect("create commands");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].display_name, "Dex");
        assert!(matches!(
            commands[0].transport,
            noema_capabilities_mcp::McpSetupTransportConfig::StreamableHttp(_)
        ));
    }
    assert!(
        store
            .control_plane_catalog()
            .await
            .expect("list servers")
            .is_empty()
    );
}

include!("mcp_oauth_tests.rs");

#[derive(Default)]
struct McpBoundaryOperations {
    create_commands: Mutex<Vec<CreateMcpServerCommand>>,
}

impl McpBoundaryOperations {
    fn without_runtime() -> Self {
        Self::default()
    }
}

macro_rules! failed_operation_method {
    ($method:ident($($argument:ty),*) -> $result:ty) => {
        fn $method(&self $(, _: $argument)*) -> McpOperationFuture<'_, McpOperationResult<$result>> {
            Box::pin(async { Err(McpOperationError::Failed) })
        }
    };
}

impl McpOperations for McpBoundaryOperations {
    failed_operation_method!(list_servers() -> McpServerList);
    failed_operation_method!(list_tools(McpListToolsCommand) -> McpToolList);

    fn create_server(
        &self,
        command: CreateMcpServerCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpServerSetupResult>> {
        self.create_commands
            .lock()
            .expect("create commands")
            .push(command);
        Box::pin(async {
            Ok(McpServerSetupResult {
                server: None,
                setup_status: McpSetupStatus::NeedsAuth,
                discovery_status: Some(McpDiscoveryStatus::NeedsAuth),
                discovered_tool_count: 0,
                issue: Some(McpSetupIssue::AuthenticationRequired),
                auth: Some(McpSetupAuthDetails {
                    oauth_client_credentials_supported: true,
                    oauth_authorization_supported: true,
                    scopes: Vec::new(),
                }),
            })
        })
    }

    failed_operation_method!(add_connection(AddMcpConnectionCommand) -> McpServerSetupResult);
    failed_operation_method!(continue_setup(ContinueMcpServerSetupCommand) -> McpServerSetupResult);
    failed_operation_method!(start_oauth_setup(StartMcpOAuthSetupCommand) -> McpOAuthSetupAttemptView);
    failed_operation_method!(
        start_oauth_reauthentication(StartMcpOAuthReauthenticationCommand) -> McpOAuthSetupAttemptView
    );
    failed_operation_method!(oauth_setup_attempt(McpOAuthSetupAttemptQuery) -> Option<McpOAuthSetupAttemptView>);
    fn complete_oauth_setup(
        &self,
        command: CompleteMcpOAuthSetupCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpOAuthSetupAttemptView>> {
        Box::pin(async move {
            Ok(McpOAuthSetupAttemptView {
                attempt_id: command.attempt_id,
                status: McpOAuthSetupAttemptStatus::Completed,
                authorization_url: None,
                setup_result: None,
                failure: None,
            })
        })
    }

    failed_operation_method!(save_provider_policy(McpSaveProviderPolicyCommand) -> McpServerRecord);
    failed_operation_method!(save_tool_override(McpSaveToolOverrideCommand) -> McpToolPolicyRecord);
    failed_operation_method!(reset_tool_policy(McpResetToolPolicyCommand) -> McpToolPolicyRecord);
    failed_operation_method!(set_tool_enabled(McpSetToolEnabledCommand) -> McpToolPolicyRecord);
    failed_operation_method!(delete_server(McpDeleteServerCommand) -> McpDeleteServerResult);
}
