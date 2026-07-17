use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, PoisonError},
};

use noema_capabilities_mcp::{
    CompleteMcpOAuthSetupCommand, ContinueMcpServerSetupCommand, CreateMcpServerCommand,
    McpAutofillCalibrationsCommand, McpAutofillCalibrationsResult, McpControlPlaneHandle,
    McpDeleteServerCommand, McpDeleteServerResult, McpDiscoveredTool, McpDiscoveryStatus,
    McpInitialDiscoveryCommit, McpListToolsCommand, McpOAuthSetupAttemptQuery,
    McpOAuthSetupAttemptView, McpOperationError, McpOperationFuture, McpOperations, McpRepository,
    McpSaveCalibrationsCommand, McpSaveCalibrationsResult, McpServerAuthStatus, McpServerList,
    McpServerSetupResult, McpSetupAuthDetails, McpSetupIssue, McpSetupStatus,
    McpSetupTransportConfig, McpToolList, NewMcpServer, StartMcpOAuthReauthenticationCommand,
    StartMcpOAuthSetupCommand, build_autofill_prompt, discovered_tool_fingerprint,
    parse_autofill_response,
};
use noema_runtime::RuntimeHandle;
use noema_store::NoemaStore;

#[derive(Clone, Debug)]
pub(crate) enum TestMcpSetupOutcome {
    Ok(Vec<McpDiscoveredTool>),
    AuthRequired(String),
}

pub(crate) fn test_mcp_operations(
    store: NoemaStore,
    runtime: Option<RuntimeHandle>,
) -> McpControlPlaneHandle {
    Arc::new(TestStoreMcpOperations {
        store,
        runtime,
        environment: None,
        setup_outcomes: Mutex::new(VecDeque::new()),
    })
}

pub(crate) fn test_mcp_operations_with_setup(
    store: NoemaStore,
    runtime: Option<RuntimeHandle>,
    environment: super::TestEnvironment,
    outcomes: Vec<TestMcpSetupOutcome>,
) -> McpControlPlaneHandle {
    Arc::new(TestStoreMcpOperations {
        store,
        runtime,
        environment: Some(environment),
        setup_outcomes: Mutex::new(VecDeque::from(outcomes)),
    })
}

struct TestStoreMcpOperations {
    store: NoemaStore,
    runtime: Option<RuntimeHandle>,
    environment: Option<super::TestEnvironment>,
    setup_outcomes: Mutex<VecDeque<TestMcpSetupOutcome>>,
}

impl TestStoreMcpOperations {
    async fn create(
        &self,
        command: CreateMcpServerCommand,
    ) -> Result<McpServerSetupResult, McpOperationError> {
        validate_safe_transport(&command.transport)?;
        let outcome = self
            .setup_outcomes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| TestMcpSetupOutcome::Ok(Vec::new()));
        match outcome {
            TestMcpSetupOutcome::AuthRequired(_message) => Ok(McpServerSetupResult {
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
            }),
            TestMcpSetupOutcome::Ok(mut tools) => {
                for tool in &mut tools {
                    if tool.metadata_fingerprint.is_empty() {
                        tool.metadata_fingerprint = discovered_tool_fingerprint(tool);
                    }
                }
                let discovered_tool_count = tools.len();
                let has_secrets = command.secrets.has_secret_material();
                let server = self
                    .store
                    .commit_initial_discovery(McpInitialDiscoveryCommit {
                        server: NewMcpServer {
                            display_name: command.display_name,
                            transport_kind: command.transport.transport_kind(),
                            safe_config: command.transport.safe_config(),
                        },
                        tools,
                        auth_status: if has_secrets {
                            McpServerAuthStatus::Authenticated
                        } else {
                            McpServerAuthStatus::None
                        },
                    })
                    .await
                    .map_err(|_| McpOperationError::Unavailable)?;
                if has_secrets {
                    self.write_secret_marker(&server.server.mcp_server_id)?;
                }
                Ok(McpServerSetupResult {
                    server: Some(server.server),
                    setup_status: McpSetupStatus::ReadyForCalibration,
                    discovery_status: Some(McpDiscoveryStatus::Discovered),
                    discovered_tool_count,
                    issue: None,
                    auth: None,
                })
            }
        }
    }

    fn write_secret_marker(&self, mcp_server_id: &str) -> Result<(), McpOperationError> {
        let Some(environment) = &self.environment else {
            return Ok(());
        };
        let home = environment.mcp_server_home(mcp_server_id);
        std::fs::create_dir_all(&home).map_err(|_| McpOperationError::Unavailable)?;
        std::fs::write(home.join("secrets.json"), b"{}").map_err(|_| McpOperationError::Unavailable)
    }
}

impl McpOperations for TestStoreMcpOperations {
    fn list_servers(&self) -> McpOperationFuture<'_, Result<McpServerList, McpOperationError>> {
        Box::pin(async move {
            self.store
                .control_plane_catalog()
                .await
                .map(|servers| McpServerList {
                    servers: servers.into_iter().map(|server| server.server).collect(),
                })
                .map_err(|_| McpOperationError::Unavailable)
        })
    }

    fn list_tools(
        &self,
        command: McpListToolsCommand,
    ) -> McpOperationFuture<'_, Result<McpToolList, McpOperationError>> {
        Box::pin(async move {
            let server = self
                .store
                .control_plane_server(command.mcp_server_id)
                .await
                .map_err(|_| McpOperationError::Unavailable)?
                .ok_or(McpOperationError::NotFound)?;
            Ok(McpToolList {
                server: server.server,
                tools: server.tools,
            })
        })
    }

    fn create_server(
        &self,
        command: CreateMcpServerCommand,
    ) -> McpOperationFuture<'_, Result<McpServerSetupResult, McpOperationError>> {
        Box::pin(async move { self.create(command).await })
    }

    fn continue_setup(
        &self,
        _command: ContinueMcpServerSetupCommand,
    ) -> McpOperationFuture<'_, Result<McpServerSetupResult, McpOperationError>> {
        Box::pin(async { Err(McpOperationError::Failed) })
    }

    fn start_oauth_setup(
        &self,
        _command: StartMcpOAuthSetupCommand,
    ) -> McpOperationFuture<'_, Result<McpOAuthSetupAttemptView, McpOperationError>> {
        Box::pin(async { Err(McpOperationError::Failed) })
    }

    fn start_oauth_reauthentication(
        &self,
        _command: StartMcpOAuthReauthenticationCommand,
    ) -> McpOperationFuture<'_, Result<McpOAuthSetupAttemptView, McpOperationError>> {
        Box::pin(async { Err(McpOperationError::Failed) })
    }

    fn oauth_setup_attempt(
        &self,
        _query: McpOAuthSetupAttemptQuery,
    ) -> McpOperationFuture<'_, Result<Option<McpOAuthSetupAttemptView>, McpOperationError>> {
        Box::pin(async { Ok(None) })
    }

    fn complete_oauth_setup(
        &self,
        _command: CompleteMcpOAuthSetupCommand,
    ) -> McpOperationFuture<'_, Result<McpOAuthSetupAttemptView, McpOperationError>> {
        Box::pin(async { Err(McpOperationError::Failed) })
    }

    fn autofill_calibrations(
        &self,
        command: McpAutofillCalibrationsCommand,
    ) -> McpOperationFuture<'_, Result<McpAutofillCalibrationsResult, McpOperationError>> {
        Box::pin(async move {
            let runtime = self
                .runtime
                .as_ref()
                .ok_or(McpOperationError::Unavailable)?;
            let server = self
                .store
                .control_plane_server(command.mcp_server_id)
                .await
                .map_err(|_| McpOperationError::Unavailable)?
                .ok_or(McpOperationError::NotFound)?;
            let tools = server
                .tools
                .iter()
                .map(|tool| tool.tool.clone())
                .collect::<Vec<_>>();
            let prompt = build_autofill_prompt(&server.server.display_name, &tools);
            let mut request = noema_providers::GenerateRequest::text(prompt);
            request.instructions =
                Some("Return strict JSON only for MCP calibration suggestions.".to_string());
            let response = runtime
                .generate_once_with_tool_classification_model(request)
                .await
                .map_err(|_| McpOperationError::Failed)?;
            let suggestions = parse_autofill_response(&response.assistant_text(), &tools)
                .map_err(|_| McpOperationError::MalformedResponse)?;
            Ok(McpAutofillCalibrationsResult { suggestions })
        })
    }

    fn save_calibrations(
        &self,
        command: McpSaveCalibrationsCommand,
    ) -> McpOperationFuture<'_, Result<McpSaveCalibrationsResult, McpOperationError>> {
        Box::pin(async move {
            self.store
                .save_calibrations(command.calibrations)
                .await
                .map(|calibrations| McpSaveCalibrationsResult { calibrations })
                .map_err(|_| McpOperationError::InvalidInput)
        })
    }

    fn delete_server(
        &self,
        command: McpDeleteServerCommand,
    ) -> McpOperationFuture<'_, Result<McpDeleteServerResult, McpOperationError>> {
        Box::pin(async move {
            let mcp_server_id = command.mcp_server_id;
            let Some(ticket) = self
                .store
                .begin_delete(mcp_server_id.clone())
                .await
                .map_err(|_| McpOperationError::Unavailable)?
            else {
                return Ok(McpDeleteServerResult { deleted: false });
            };
            if let Some(environment) = &self.environment {
                let home = environment.mcp_server_home(&mcp_server_id);
                if home.exists() {
                    std::fs::remove_dir_all(home).map_err(|_| McpOperationError::Unavailable)?;
                }
            }
            let deleted = self
                .store
                .finish_delete(ticket)
                .await
                .map_err(|_| McpOperationError::Unavailable)?;
            Ok(McpDeleteServerResult { deleted })
        })
    }
}

fn validate_safe_transport(transport: &McpSetupTransportConfig) -> Result<(), McpOperationError> {
    let values = match transport {
        McpSetupTransportConfig::Stdio(config) => &config.env,
        McpSetupTransportConfig::StreamableHttp(config) => &config.headers,
    };
    reject_secret_shaped_keys(values)
}

fn reject_secret_shaped_keys(values: &BTreeMap<String, String>) -> Result<(), McpOperationError> {
    const SECRET_MARKERS: [&str; 11] = [
        "authorization",
        "token",
        "api_key",
        "apikey",
        "cookie",
        "password",
        "credential",
        "secret",
        "private_key",
        "session",
        "set-cookie",
    ];
    if values.keys().any(|key| {
        let normalized = key.to_ascii_lowercase();
        SECRET_MARKERS
            .iter()
            .any(|marker| normalized.contains(marker))
    }) {
        Err(McpOperationError::InvalidInput)
    } else {
        Ok(())
    }
}
