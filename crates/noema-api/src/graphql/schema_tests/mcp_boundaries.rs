use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use noema_capabilities_mcp::{
    CompleteMcpOAuthSetupCommand, ContinueMcpServerSetupCommand, CreateMcpServerCommand,
    McpAutofillCalibrationsCommand, McpAutofillCalibrationsResult, McpDeleteServerCommand,
    McpDeleteServerResult, McpDiscoveryStatus, McpListToolsCommand, McpOAuthSetupAttemptQuery,
    McpOAuthSetupAttemptView, McpOperationError, McpOperationFuture, McpOperationResult,
    McpOperations, McpSaveCalibrationsCommand, McpSaveCalibrationsResult, McpServerList,
    McpServerSetupResult, McpSetupAuthDetails, McpSetupIssue, McpSetupStatus, McpToolList,
    StartMcpOAuthReauthenticationCommand, StartMcpOAuthSetupCommand,
};

#[tokio::test]
async fn mcp_graphql_route_and_setup_boundaries() {
    // Case: autofill_tool_calibrations_reads_classification_model_from_replacement_route.
    use crate::test_support::{spawn_runtime_with_provider_registry, test_store};

    let store = test_store().await;
    let account = crate::test_support::authenticated_default_provider(&store).await;
    let instance_key = noema_providers::provider_account_instance_key(&account.provider_account_id)
        .expect("provider instance key");
    let mut selection = noema_providers::ProviderSelectionSnapshot::explicit(
        "codex",
        &account.provider_account_id,
        "gpt-test",
        None,
        Some("test_configured_default".to_string()),
    );
    selection.provider_instance_key = Some(instance_key.clone());

    let registry = Arc::new(noema_providers::ProviderRegistry::new());
    let old_requests = Arc::new(Mutex::new(Vec::new()));
    registry
        .register(
            instance_key.clone(),
            noema_providers::erase_model_provider(RecordingProvider {
                classification_model: "old-classifier".to_string(),
                requests: old_requests.clone(),
            }),
        )
        .expect("register old provider");
    let ready = registry
        .prove_ready_selection(selection.clone())
        .expect("ready selection");
    store
        .initialize_missing_provider_selections(&selection, Some(&ready))
        .await
        .expect("initialize provider selections");
    let runtime = spawn_runtime_with_provider_registry(registry.clone(), store.clone())
        .await
        .expect("runtime");

    let replacement_requests = Arc::new(Mutex::new(Vec::new()));
    registry
        .register(
            instance_key,
            noema_providers::erase_model_provider(RecordingProvider {
                classification_model: "replacement-classifier".to_string(),
                requests: replacement_requests.clone(),
            }),
        )
        .expect("publish replacement");
    let operations = Arc::new(McpBoundaryOperations::with_runtime(runtime.clone()));
    let schema = build_schema(
        GraphqlState::for_tests_with_store_and_runtime(store, runtime.clone())
            .with_mcp_operations(operations.clone()),
    );

    let response = schema
        .execute(
            r#"mutation {
              autofillToolCalibrations(mcpServerId: "mcp_server:replacement-route") {
                suggestions { mcpToolId }
              }
            }"#,
        )
        .await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(operations.autofill_calls.load(Ordering::SeqCst), 1);
    assert!(old_requests.lock().expect("old requests").is_empty());
    {
        let replacement_requests = replacement_requests.lock().expect("replacement requests");
        assert_eq!(replacement_requests.len(), 1);
        assert_eq!(
            replacement_requests[0].model.as_deref(),
            Some("replacement-classifier")
        );
    }
    runtime.shutdown().await;

    // Case: create_mcp_server_mutation_reports_browser_oauth_before_persisting.
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

#[derive(Debug)]
struct RecordingProvider {
    classification_model: String,
    requests: Arc<Mutex<Vec<noema_providers::GenerateRequest>>>,
}

impl noema_providers::ModelProvider for RecordingProvider {
    async fn generate(
        &self,
        request: noema_providers::GenerateRequest,
    ) -> Result<noema_providers::GenerateResponse, noema_providers::ProviderError> {
        self.requests.lock().expect("requests").push(request);
        Ok(noema_providers::GenerateResponse::final_text(
            "{}",
            "test",
            "test-autofill",
        ))
    }

    fn default_tool_classification_model(&self) -> Option<String> {
        Some(self.classification_model.clone())
    }
}

#[derive(Default)]
struct McpBoundaryOperations {
    runtime: Option<noema_runtime::RuntimeHandle>,
    autofill_calls: AtomicUsize,
    create_commands: Mutex<Vec<CreateMcpServerCommand>>,
}

impl McpBoundaryOperations {
    fn with_runtime(runtime: noema_runtime::RuntimeHandle) -> Self {
        Self {
            runtime: Some(runtime),
            ..Self::default()
        }
    }

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

    failed_operation_method!(continue_setup(ContinueMcpServerSetupCommand) -> McpServerSetupResult);
    failed_operation_method!(start_oauth_setup(StartMcpOAuthSetupCommand) -> McpOAuthSetupAttemptView);
    failed_operation_method!(
        start_oauth_reauthentication(StartMcpOAuthReauthenticationCommand) -> McpOAuthSetupAttemptView
    );
    failed_operation_method!(oauth_setup_attempt(McpOAuthSetupAttemptQuery) -> Option<McpOAuthSetupAttemptView>);
    failed_operation_method!(complete_oauth_setup(CompleteMcpOAuthSetupCommand) -> McpOAuthSetupAttemptView);

    fn autofill_calibrations(
        &self,
        _command: McpAutofillCalibrationsCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpAutofillCalibrationsResult>> {
        self.autofill_calls.fetch_add(1, Ordering::SeqCst);
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let runtime = runtime.ok_or(McpOperationError::Unavailable)?;
            runtime
                .generate_once_with_tool_classification_model(
                    noema_providers::GenerateRequest::text("classify MCP metadata"),
                )
                .await
                .map_err(|_| McpOperationError::Failed)?;
            Ok(McpAutofillCalibrationsResult {
                suggestions: Vec::new(),
            })
        })
    }

    failed_operation_method!(save_calibrations(McpSaveCalibrationsCommand) -> McpSaveCalibrationsResult);
    failed_operation_method!(delete_server(McpDeleteServerCommand) -> McpDeleteServerResult);
}
